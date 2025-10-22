//! Tower Service implementation for TFLService.
//!
//! This module integrates `TFLServiceHandle` with the `tower::Service` trait,
//! allowing it to handle asynchronous service requests.

use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::task::{Context, Poll};

use ed25519_zebra::VerificationKeyBytes;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;

use tracing::{error, info, warn};

use zebra_chain::block::{Hash as BlockHash, Height as BlockHeight};
use zebra_chain::transaction::Hash as TxHash;
use zebra_state::{crosslink::*, Request as StateRequest, Response as StateResponse};

use crate::chain::BftBlock;
use crate::malctx::{MalPublicKey2, MalValidator};
use crate::FatPointerToBftBlock2;
use crate::{
    rng_private_public_key_from_address, tfl_service_incoming_request, TFLBlockFinality, TFLRoster,
    TFLServiceInternal,
};

use tower::Service;
impl Service<TFLServiceRequest> for TFLServiceHandle {
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

/// A pinned-in-memory, heap-allocated, reference-counted, thread-safe, asynchronous function
/// pointer that takes a `StateRequest` as input and returns a `StateResponse` as output.
///
/// The error is boxed to allow for dynamic error types.
pub(crate) type StateServiceProcedure = Arc<
    dyn Fn(
            StateRequest,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<StateResponse, Box<dyn std::error::Error + Send + Sync>>>
                    + Send,
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
    pub(crate) state: StateServiceProcedure,
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
/// - `state_service_call` takes a [`StateRequest`] as input and returns a [`StateResponse`] as output.
///
/// [`TFLServiceHandle`] is a shallow handle that can be cloned and passed between threads.
pub fn spawn_new_tfl_service(
    is_regtest: bool,
    state_service_call: StateServiceProcedure,
    force_feed_pow_call: ForceFeedPoWBlockProcedure,
    config: crate::config::Config,
) -> (TFLServiceHandle, JoinHandle<Result<(), String>>) {
    let (validators_at_current_height, validators_keys_to_names) = {
        let mut array = Vec::with_capacity(config.malachite_peers.len());
        let mut map = std::collections::HashMap::with_capacity(config.malachite_peers.len());

        for peer in config.malachite_peers.iter() {
            let (_, _, public_key) = rng_private_public_key_from_address(peer.as_bytes());
            array.push(MalValidator::new(public_key, 1));
            map.insert(public_key, peer.to_string());
        }

        if array.is_empty() {
            let public_ip_string = config
                .public_address
                .clone()
                .unwrap_or(String::from_str("/ip4/127.0.0.1/udp/45869/quic-v1").unwrap());
            let user_name = config
                .insecure_user_name
                .clone()
                .unwrap_or(public_ip_string);
            // .unwrap_or(String::from_str("tester").unwrap());
            info!("user_name: {}", user_name);
            let (_, _, public_key) = rng_private_public_key_from_address(&user_name.as_bytes());
            array.push(MalValidator::new(public_key, 1));
            map.insert(public_key, user_name);
        }

        (array, map)
    };

    let internal = Arc::new(Mutex::new(TFLServiceInternal {
        my_public_key: VerificationKeyBytes::from([0u8; 32]),
        latest_final_block: None,
        tfl_is_activated: if is_regtest { true } else { false },
        stakers: Vec::new(),
        final_change_tx: broadcast::channel(16).0,
        bft_msg_flags: 0,
        bft_err_flags: 0,
        bft_blocks: Vec::new(),
        fat_pointer_to_tip: FatPointerToBftBlock2::null(),
        our_set_bft_string: None,
        active_bft_string: None,
        #[cfg(feature = "malachite")]
        malachite_watchdog: tokio::time::Instant::now(),
        validators_at_current_height,
        validators_keys_to_names,
        current_bc_final: None,
    }));

    let handle_mtx = Arc::new(std::sync::Mutex::new(None));

    let handle_mtx2 = handle_mtx.clone();
    let force_feed_pos: ForceFeedPoSBlockProcedure = Arc::new(move |block, fat_pointer| {
        let handle = handle_mtx2.lock().unwrap().clone().unwrap();
        Box::pin(async move {
            #[cfg(feature = "malachite")]
            let (accepted, _) =
                crate::new_decided_bft_block_from_malachite(&handle, block.as_ref(), &fat_pointer)
                    .await;
            #[cfg(not(feature = "malachite"))]
            let accepted = if fat_pointer.points_at_block_hash() == block.blake3_hash() {
                crate::validate_bft_block_from_malachite(&handle, block.as_ref()).await
                    == tenderlink::TMStatus::Pass
            } else {
                false
            };
            if accepted {
                info!("Successfully force-fed BFT block");
                #[cfg(not(feature = "malachite"))]
                crate::new_decided_bft_block_from_malachite(&handle, block.as_ref(), &fat_pointer)
                    .await;
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
            state: state_service_call,
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
