//! Tower Service implementation for TFLService.
//!
//! This module integrates `TFLServiceHandle` with the `tower::Service` trait,
//! allowing it to handle asynchronous service requests.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;

use tracing::{error, info, warn};

use zebra_chain::block::{Hash as BlockHash, Height as BlockHeight};
use zebra_chain::transaction::Hash as TxHash;
use zebra_state::{ReadRequest as ReadStateRequest, ReadResponse as ReadStateResponse};

use crate::chain::BftBlock;
use crate::mal_system::FatPointerToBftBlock2;
use crate::malctx::MalValidator;
use crate::{
    rng_private_public_key_from_address, tfl_service_incoming_request, TFLBlockFinality, TFLRoster, TFLServiceInternal, TFLStaker
};

impl tower::Service<TFLServiceRequest> for TFLServiceHandle {
    type Response = TFLServiceResponse;
    type Error = TFLServiceError;
    type Future = Pin<Box<dyn Future<Output = Result<TFLServiceResponse, TFLServiceError>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: TFLServiceRequest) -> Self::Future {
        let duplicate_handle = self.clone();
        Box::pin(async move { tfl_service_incoming_request(duplicate_handle, request).await })
    }
}

/// Types of requests that can be made to the TFLService.
///
/// These map one to one to the variants of the same name in [`TFLServiceResponse`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TFLServiceRequest {
    /// Is the TFL service activated yet?
    IsTFLActivated,
    /// Get the final block hash
    FinalBlockHash,
    /// Get a receiver for the final block hash
    FinalBlockRx,
    /// Set final block hash
    SetFinalBlockHash(BlockHash),
    /// Get the finality status of a block
    BlockFinalityStatus(BlockHash),
    /// Get the finality status of a transaction
    TxFinalityStatus(TxHash),
    /// Get the finalizer roster
    Roster,
    /// Update the list of stakers
    UpdateStaker(TFLStaker),
    /// Get the fat pointer to the BFT chain tip
    FatPointerToBFTChainTip,
}

/// Types of responses that can be returned by the TFLService.
///
/// These map one to one to the variants of the same name in [`TFLServiceRequest`].
#[derive(Debug)]
pub enum TFLServiceResponse {
    /// Is the TFL service activated yet?
    IsTFLActivated(bool),
    /// Final block hash
    FinalBlockHash(Option<BlockHash>),
    /// Receiver for the final block hash
    FinalBlockRx(broadcast::Receiver<BlockHash>),
    /// Set final block hash
    SetFinalBlockHash(Option<BlockHeight>),
    /// Finality status of a block
    BlockFinalityStatus(Option<TFLBlockFinality>),
    /// Finality status of a transaction
    TxFinalityStatus(Option<TFLBlockFinality>),
    /// Finalizer roster
    Roster(TFLRoster),
    /// Update the list of stakers
    UpdateStaker, // TODO: batch?
    /// Fat pointer to the BFT chain tip
    FatPointerToBFTChainTip(zebra_chain::block::FatPointerToBftBlock),
}

/// Errors that can occur when interacting with the TFLService.
#[derive(Debug)]
pub enum TFLServiceError {
    /// Not implemented error
    NotImplemented,
    /// Arbitrary error
    Misc(String),
}

impl fmt::Display for TFLServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TFLServiceError: {:?}", self)
    }
}

impl std::error::Error for TFLServiceError {}

/// A pinned-in-memory, heap-allocated, reference-counted, thread-safe, asynchronous function
/// pointer that takes a `ReadStateRequest` as input and returns a `ReadStateResponse` as output.
///
/// The error is boxed to allow for dynamic error types.
pub(crate) type ReadStateServiceProcedure = Arc<
    dyn Fn(
            ReadStateRequest,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            ReadStateResponse,
                            Box<dyn std::error::Error + Send + Sync>,
                        >,
                    > + Send,
            >,
        > + Send
        + Sync,
>;

/// A pinned-in-memory, heap-allocated, reference-counted, thread-safe, asynchronous function
/// pointer that takes an `Arc<Block>` as input and returns `()` as its output.
pub(crate) type ForceFeedPoWBlockProcedure = Arc<
    dyn Fn(Arc<zebra_chain::block::Block>) -> Pin<Box<dyn Future<Output = bool> + Send>>
        + Send
        + Sync,
>;

/// A pinned-in-memory, heap-allocated, reference-counted, thread-safe, asynchronous function
/// pointer that takes an `Arc<Block>` as input and returns `()` as its output.
pub(crate) type ForceFeedPoSBlockProcedure = Arc<
    dyn Fn(Arc<BftBlock>, FatPointerToBftBlock2) -> Pin<Box<dyn Future<Output = bool> + Send>>
        + Send
        + Sync,
>;

/// `TFLServiceCalls` encapsulates the service calls that this service needs to make to other services.
/// Simply put, it is a function pointer bundle for all outgoing calls to the rest of Zebra.
#[derive(Clone)]
pub struct TFLServiceCalls {
    pub(crate) read_state: ReadStateServiceProcedure,
    pub(crate) force_feed_pow: ForceFeedPoWBlockProcedure,
    pub(crate) force_feed_pos: ForceFeedPoSBlockProcedure,
}
impl fmt::Debug for TFLServiceCalls {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TFLServiceCalls")
    }
}

/// Spawn a Trailing Finality Service that uses the provided
/// closures to call out to other services.
///
/// - `read_state_service_call` takes a [`ReadStateRequest`] as input and returns a [`ReadStateResponse`] as output.
///
/// [`TFLServiceHandle`] is a shallow handle that can be cloned and passed between threads.
pub fn spawn_new_tfl_service(
    read_state_service_call: ReadStateServiceProcedure,
    force_feed_pow_call: ForceFeedPoWBlockProcedure,
    config: crate::config::Config,
) -> (TFLServiceHandle, JoinHandle<Result<(), String>>) {
    let internal = Arc::new(Mutex::new(TFLServiceInternal {
        latest_final_block: None,
        tfl_is_activated: false,
        stakers: Vec::new(),
        final_change_tx: broadcast::channel(16).0,
        bft_msg_flags: 0,
        bft_blocks: Vec::new(),
        fat_pointer_to_tip: FatPointerToBftBlock2::null(),
        proposed_bft_string: None,
        malachite_watchdog: tokio::time::Instant::now(),
        validators_at_current_height: {
            let mut array = Vec::with_capacity(config.malachite_peers.len());

            for peer in config.malachite_peers.iter() {
                let (_, _, public_key) = rng_private_public_key_from_address(peer.as_bytes());
                array.push(MalValidator::new(public_key, 1));
            }

            if array.is_empty() {
                let public_ip_string = config
                    .public_address.clone()
                    .unwrap_or(String::from_str("/ip4/127.0.0.1/udp/45869/quic-v1").unwrap());
                let (_, _, public_key) = rng_private_public_key_from_address(&public_ip_string.as_bytes());
                array.push(MalValidator::new(public_key, 1));
            }

            array
        },
    }));

    let handle_mtx = Arc::new(std::sync::Mutex::new(None));

    let handle_mtx2 = handle_mtx.clone();
    let force_feed_pos: ForceFeedPoSBlockProcedure = Arc::new(move |block, fat_pointer| {
        let handle = handle_mtx2.lock().unwrap().clone().unwrap();
        Box::pin(async move {
            let (accepted, _) = crate::new_decided_bft_block_from_malachite(&handle, block.as_ref(), &fat_pointer).await;
            if accepted {
                info!("Successfully force-fed BFT block");
                true
            } else {
                error!("Failed to force-feed BFT block");
                false
            }
        })
    });

    let handle1 = TFLServiceHandle {
        internal,
        call: TFLServiceCalls {
            read_state: read_state_service_call,
            force_feed_pow: force_feed_pow_call,
            force_feed_pos,
        },
        config,
    };

    *handle_mtx.lock().unwrap() = Some(handle1.clone());

    let handle2 = handle1.clone();

    (
        handle1,
        tokio::spawn(async move { crate::tfl_service_main_loop(handle2).await }),
    )
}

/// A wrapper around the `TFLServiceInternal` and `TFLServiceCalls` types, used to manage
/// the internal state of the TFLService and the service calls that can be made to it.
#[derive(Clone, Debug)]
pub struct TFLServiceHandle {
    /// A threadsafe wrapper around the stored internal data
    pub(crate) internal: Arc<Mutex<TFLServiceInternal>>,
    /// The collection of service calls available
    pub(crate) call: TFLServiceCalls,
    /// The file-generated config data
    pub config: crate::config::Config,
}
