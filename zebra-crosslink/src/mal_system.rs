
use std::{panic::AssertUnwindSafe, sync::Weak};

use futures::FutureExt;

use super::*;

pub struct RunningMalachite {
    pub should_terminate: bool,
    pub engine_handle: EngineHandle,

    pub height: u64,
    pub validator_set: Vec<MalValidator>,
}
impl Drop for RunningMalachite {
    fn drop(&mut self) {
        self.engine_handle.actor.stop(None);
    }
}

pub async fn start_malachite(tfl_handle: TFLServiceHandle, at_height: u64, validators_at_height: Vec<MalValidator>, my_private_key: MalPrivateKey, bft_config: BFTConfig) -> Arc<TokioMutex<RunningMalachite>>
{
    let ctx = MalContext {};
    let codec = MalProtobufCodec;
    let bft_node_handle = BFTNode {
        private_key: my_private_key.clone(),
    };

    {
        let mut td = TEMP_DIR_FOR_WAL.lock().unwrap();
        *td = Some(tempfile::Builder::new()
                    .prefix(&format!(
                        "aah_very_annoying_that_the_wal_is_required_id_is_{}",
                        rand::random::<u32>()
                    ))
                    .tempdir()
                    .unwrap());
    }

    let (channels, engine_handle) = malachitebft_app_channel::start_engine(
        ctx,
        codec,
        bft_node_handle,
        bft_config,
        Some(MalHeight::new(at_height)),
        MalValidatorSet { validators: validators_at_height.clone(), },
    )
    .await
    .unwrap();

    let arc_handle = Arc::new(TokioMutex::new(RunningMalachite {
        should_terminate: false,
        engine_handle,

        height: at_height,
        validator_set: validators_at_height,
    }));

    let weak_self = Arc::downgrade(&arc_handle);
    tokio::spawn(async move {
        match AssertUnwindSafe(malachite_system_main_loop(tfl_handle, weak_self, channels)).catch_unwind().await {
            Ok(()) => (),
            Err(error) => {
                eprintln!("panic occurred inside mal_system.rs: {:?}", error);
                std::process::abort();
            }
        }
    });
    arc_handle
}

async fn malachite_system_main_loop(tfl_handle: TFLServiceHandle, weak_self: Weak<TokioMutex<RunningMalachite>>, mut channels: Channels<MalContext>) {
    loop {
        let running_malachite = if let Some(arc) = weak_self.upgrade() { arc } else { return; };
        
        let mut something_to_do_maybe = None;
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            ret = channels.consensus.recv() => { something_to_do_maybe = ret; }
        };

        let lock = running_malachite.lock().await;
        if lock.should_terminate { lock.engine_handle.actor.stop(None); return; }

        if something_to_do_maybe.is_none() { continue; }
        let app_msg = something_to_do_maybe.unwrap();

        let mut bft_msg_flags = 0;
        match app_msg {
            BFTAppMsg::ConsensusReady { reply } => {
                bft_msg_flags |= 1 << BFTMsgFlag::ConsensusReady as u64;
                reply.send((MalHeight::new(lock.height), MalValidatorSet { validators: lock.validator_set.clone(), })).unwrap();
            },
            _ => (), //panic!("AppMsg variant not handled: {:?}", app_msg),
        }
    }
}
