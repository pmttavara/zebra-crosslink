
use std::{ops::Deref, panic::AssertUnwindSafe, sync::Weak};

use color_eyre::owo_colors::OwoColorize;
use futures::FutureExt;
use tracing_subscriber::fmt::format::PrettyVisitor;

use crate::malctx::malctx_schema_proto::CommitCertificate;

use super::*;

/*

TODO LIST
DONE 1. Make sure multiplayer works.
2. Remove streaming and massively simplify how payloads are communicated. And also refactor the MalValue to just be an array of bytes.
DONE 3. Sledgehammer in malachite to allow the vote extension scheme that we want.
4. Upgrade to the double rate scheme in order to make blocks with canonical votes on them. Also define the BFTChainTipEvidence type.
5. Make sure that syncing within malachite works.
6. Make sure the payload has the Roster for H + 1 for easy light client validation.
7. Lock in the formats and bake out some tests.
8. See if we can forget proposals for lower rounds than the current round. Then stop at_this_height_previously_seen_proposals from being an infinitely growing array.

*/


pub struct RunningMalachite {
    pub should_terminate: bool,
    pub engine_handle: EngineHandle,

    pub height: u64,
    pub round: u32,
    pub validator_set: Vec<MalValidator>,
}
impl Drop for RunningMalachite {
    fn drop(&mut self) {
        self.engine_handle.actor.stop(None);
    }
}

pub async fn start_malachite(tfl_handle: TFLServiceHandle, at_height: u64, validators_at_height: Vec<MalValidator>, my_private_key: MalPrivateKey, bft_config: BFTConfig,) -> Arc<TokioMutex<RunningMalachite>>
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
        round: 0,
        validator_set: validators_at_height,
    }));

    let weak_self = Arc::downgrade(&arc_handle);
    tokio::spawn(async move {
        match AssertUnwindSafe(malachite_system_main_loop(tfl_handle, weak_self, channels, my_private_key)).catch_unwind().await {
            Ok(()) => (),
            Err(error) => {
                eprintln!("panic occurred inside mal_system.rs: {:?}", error);
                std::process::abort();
            }
        }
    });
    arc_handle
}

async fn malachite_system_main_loop(tfl_handle: TFLServiceHandle, weak_self: Weak<TokioMutex<RunningMalachite>>, mut channels: Channels<MalContext>, my_private_key: MalPrivateKey) {

    let codec = MalProtobufCodec;
    let my_public_key = (&my_private_key).into();
    let my_signing_provider = MalEd25519Provider::new(my_private_key.clone());

    let mut pending_block_to_push_to_core: Option<BftBlock> = None;
    let mut post_pending_block_to_push_to_core_reply: Option<tokio::sync::oneshot::Sender<ConsensusMsg<MalContext>>> = None;
    let mut post_pending_block_to_push_to_core_reply_data: Option<ConsensusMsg<MalContext>> = None;

    let mut bft_msg_flags = 0;
    let mut at_this_height_previously_seen_proposals: Vec<Option<Box<MalProposedValue<MalContext>>>> = Vec::new();

    let mut decided_certificates_by_height = HashMap::new();

    loop {
        let running_malachite = if let Some(arc) = weak_self.upgrade() { arc } else { return; };

        if let Some(pending_block) = &pending_block_to_push_to_core {
            if !new_decided_bft_block_from_malachite(&tfl_handle, pending_block).await {
                tokio::time::sleep(Duration::from_millis(800)).await;

                let mut lock = running_malachite.lock().await;
                if lock.should_terminate { lock.engine_handle.actor.stop(None); return; }
                continue;
            }
            pending_block_to_push_to_core = None;
            post_pending_block_to_push_to_core_reply.take().unwrap().send(post_pending_block_to_push_to_core_reply_data.take().unwrap()).unwrap();
        }

        let mut something_to_do_maybe = None;
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            ret = channels.consensus.recv() => { something_to_do_maybe = ret; }
        };

        let mut lock = running_malachite.lock().await;
        if lock.should_terminate { lock.engine_handle.actor.stop(None); return; }

        if something_to_do_maybe.is_none() { continue; }
        let app_msg = something_to_do_maybe.unwrap();

        #[allow(clippy::never_loop)]
        loop {
        match app_msg {
            BFTAppMsg::ConsensusReady { reply } => {
                bft_msg_flags |= 1 << BFTMsgFlag::ConsensusReady as u64;
                reply.send((MalHeight::new(lock.height), MalValidatorSet { validators: lock.validator_set.clone(), })).unwrap();
            },
            BFTAppMsg::GetHistoryMinHeight { reply } => {
                bft_msg_flags |= 1 << BFTMsgFlag::GetHistoryMinHeight as u64;
                reply.send(MalHeight::new(1)).unwrap();
            },
            BFTAppMsg::StartedRound {
                height,
                round,
                proposer,
                reply_value,
            } => {
                assert_eq!(height.as_u64(), lock.height);
                let round = round.as_u32().unwrap() as u32;

                info!(%height, %round, %proposer, "Started round");
                bft_msg_flags |= 1 << BFTMsgFlag::StartedRound as u64;
                lock.round = round;

                // If we have already built or seen a value for this height and round,
                // send it back to consensus. This may happen when we are restarting after a crash.
                let maybe_prev_seen_payload = at_this_height_previously_seen_proposals.get(round as usize).cloned().flatten();
                if let Some(proposal) = maybe_prev_seen_payload {
                    info!(%height, %round, "Replaying already known proposed value: {}", proposal.value.id());
                    reply_value.send(Some(proposal.as_ref().clone())).unwrap();
                } else {
                    reply_value.send(None).unwrap();
                }
            },
            BFTAppMsg::GetValue {
                height,
                round,
                timeout,
                reply,
            } => {
                bft_msg_flags |= 1 << BFTMsgFlag::GetValue as u64;
                let round = round.as_u32().unwrap() as usize;
                info!(%height, %round, "Consensus is requesting a value to propose. Timeout = {} ms.", timeout.as_millis());

                if pending_block_to_push_to_core.is_some() { warn!("Still waiting to push previous block."); break; }

                // Here it is important that, if we have previously built a value for this height and round,
                // we send back the very same value.
                let maybe_prev_seen_payload = at_this_height_previously_seen_proposals.get(round).map(|x| x.as_ref().map(|x| x.as_ref().clone())).flatten();

                let proposal = if let Some(proposal) = maybe_prev_seen_payload {
                    info!(%height, %round, "Replaying already known proposed value (That I proposed?): {}", proposal.value.id());

                    assert_eq!(height, proposal.height);
                    assert_eq!(round, proposal.round.as_u32().unwrap() as usize);
                    let other_type = MalLocallyProposedValue {
                        height: proposal.height,
                        round: proposal.round,
                        value: proposal.value,
                    };
                    other_type
                } else {
                    if let Some(payload) = propose_new_bft_payload(&tfl_handle, &my_public_key, lock.height).await {
                        let other_type: MalLocallyProposedValue<MalContext> = MalLocallyProposedValue {
                            height: height,
                            round: MalRound::new(round as u32),
                            value: MalValue::new(payload),
                        };
                        {
                            // The POL round is always nil when we propose a newly built value.
                            // See L15/L18 of the Tendermint algorithm.
                            let pol_round = MalRound::Nil;

                            let other_other_type = MalProposedValue {
                                height: other_type.height,
                                round: other_type.round,
                                valid_round: pol_round,
                                proposer: MalPublicKey2(my_public_key),
                                value: other_type.value.clone(),
                                validity: MalValidity::Valid,
                            };
                            while at_this_height_previously_seen_proposals.len() < round + 1 { at_this_height_previously_seen_proposals.push(None); }
                            at_this_height_previously_seen_proposals[round] = Some(Box::new(other_other_type));
                        }
                        other_type
                    } else {
                        warn!("Asked to propose but had nothing to offer!");
                        break;
                    }
                };

                reply.send(proposal.clone()).unwrap();

                // The POL round is always nil when we propose a newly built value.
                // See L15/L18 of the Tendermint algorithm.
                let pol_round = MalRound::Nil;

                // NOTE(Sam): I have inlined the code from the example so that we
                // can actually see the functionality. I am not sure what the purpose
                // of this circus is. Why not just send the value with a simple signature?
                // I am sure there is a good reason.

                // Include metadata about the proposal
                let data_bytes = proposal.value.value.zcash_serialize_to_vec().unwrap();

                let mut hasher = sha3::Keccak256::new(); // TODO(azmr): blake3/remove?
                hasher.update(proposal.height.as_u64().to_be_bytes().as_slice());
                hasher.update(proposal.round.as_i64().to_be_bytes().as_slice());
                hasher.update(&data_bytes);
                let hash = hasher.finalize().to_vec();
                let signature = my_signing_provider.sign(&hash);

                let streamed_proposal = MalStreamedProposal {
                    height: proposal.height,
                    round: proposal.round,
                    pol_round,
                    proposer: my_public_key, // @Zooko: security orange flag: should be hash of key instead; this should only be used for lookup
                    data_bytes,
                    signature,
                };

                let stream_id = {
                    let mut bytes = Vec::with_capacity(size_of::<u64>() + size_of::<u32>());
                    bytes.extend_from_slice(&height.as_u64().to_be_bytes());
                    bytes.extend_from_slice(&(round as u32).to_be_bytes());
                    malachitebft_app_channel::app::types::streaming::StreamId::new(bytes.into())
                };

                let mut msgs = vec![
                    malachitebft_app_channel::app::types::streaming::StreamMessage::new(stream_id.clone(), 0, malachitebft_app_channel::app::streaming::StreamContent::Data(streamed_proposal)),
                    malachitebft_app_channel::app::types::streaming::StreamMessage::new(stream_id, 1, malachitebft_app_channel::app::streaming::StreamContent::Fin)
                ];

                for stream_message in msgs {
                    //info!(%height, %round, "Streaming proposal part: {stream_message:?}");
                    channels
                        .network
                        .send(NetworkMsg::PublishProposalPart(stream_message))
                        .await.unwrap();
                }
            },
            BFTAppMsg::GetDecidedValue { height, reply } => {
                bft_msg_flags |= 1 << BFTMsgFlag::GetDecidedValue as u64;

                // TODO load certificate from a local lookaside buffer keyed by height that is persistant across restarts in case someone wants to sync from us.
                // This will not be necessary when we have the alternate sync mechanism that is out of band.

                // TODO Maybe we can send the block including vote extensions to the other peer.

                // let certificate = ;
                // let value_bytes = ;

                // reply.send(RawDecidedValue { certificate, value_bytes }).unwrap();
                panic!("TODO: certificates look aside buffer and payload look aside");
            },
            BFTAppMsg::ProcessSyncedValue {
                height,
                round,
                proposer,
                value_bytes,
                reply,
            } => {
                info!(%height, %round, "Processing synced value");
                bft_msg_flags |= 1 << BFTMsgFlag::ProcessSyncedValue as u64;

                let mal_value : MalValue = codec.decode(value_bytes).unwrap();
                let proposed_value = MalProposedValue {
                    height,
                    round,
                    valid_round: MalRound::Nil,
                    proposer,
                    value: mal_value,
                    validity: MalValidity::Valid,
                };
                // TODO put in local cache and the special delayed commit slot awaiting vote extensions

                // TODO if special delayed commit slot already contains a value then complete it and commit it

                // TODO validation aka error!("Could not accept synced value. I do not know of PoW hash {}", new_final_hash);

                reply.send(proposed_value).unwrap();
            },
            BFTAppMsg::GetValidatorSet { height, reply } => {
                bft_msg_flags |= 1 << BFTMsgFlag::GetValidatorSet as u64;
                if height.as_u64() == lock.height {
                    reply.send(MalValidatorSet { validators: lock.validator_set.clone() }).unwrap();
                }
            },
            BFTAppMsg::Decided {
                certificate,
                extensions,
                reply,
            } => {
                info!(
                    height = %certificate.height,
                    round = %certificate.round,
                    payload_hash = %certificate.value_id,
                    "Consensus has decided on value"
                );
                assert_eq!(lock.height, certificate.height.as_u64());
                assert_eq!(lock.round as i64, certificate.round.as_i64());
                bft_msg_flags |= 1 << BFTMsgFlag::Decided as u64;

                let decided_value = at_this_height_previously_seen_proposals[lock.round as usize].take().unwrap();
                assert_eq!(decided_value.value.id(), certificate.value_id);

                assert!(decided_certificates_by_height.insert(lock.height, certificate).is_none());

                assert!(pending_block_to_push_to_core.is_none());
                pending_block_to_push_to_core = Some(BftBlock {
                    payload: decided_value.value.value
                });
                // TODO wait for another round to canonicalize the votes.

                // TODO form completed block using the vote extensions and insert it into the push to crosslink slot variable, this way we can block while the block is invalid.

                // decided_bft_values.insert(certificate.height.as_u64(), raw_decided_value);

                lock.height += 1;
                lock.round = 0;
                at_this_height_previously_seen_proposals.clear();

                post_pending_block_to_push_to_core_reply = Some(reply);
                post_pending_block_to_push_to_core_reply_data = Some(malachitebft_app_channel::ConsensusMsg::StartHeight(MalHeight::new(lock.height), MalValidatorSet { validators: lock.validator_set.clone() }));
            },
            BFTAppMsg::ExtendVote {
                height,
                round,
                value_id,
                reply,
            } => {
                bft_msg_flags |= 1 << BFTMsgFlag::ExtendVote as u64;
                reply.send(Some(Bytes::new())).unwrap();
            },
            BFTAppMsg::VerifyVoteExtension {
                height,
                round,
                value_id,
                extension,
                reply,
            } => {
                bft_msg_flags |= 1 << BFTMsgFlag::VerifyVoteExtension as u64;
                if extension.len() == 0 {
                    reply.send(Ok(())).unwrap();
                } else {
                    reply.send(Err(MalVoteExtensionError::InvalidVoteExtension)).unwrap();
                };
            },

            // On the receiving end of these proposal parts (ie. when we are not the proposer),
            // we need to process these parts and re-assemble the full value.
            // To this end, we store each part that we receive and assemble the full value once we
            // have all its constituent parts. Then we send that value back to consensus for it to
            // consider and vote for or against it (ie. vote `nil`), depending on its validity.
            BFTAppMsg::ReceivedProposalPart { from, part, reply } => {
                bft_msg_flags |= 1 << BFTMsgFlag::ReceivedProposalPart as u64;
                let sequence = part.sequence;

                use malctx::MalStreamedProposal;
                // Check if we have a full proposal
                let peer_id = from;
                let msg = part;

                if let Some(parts) = msg.content.as_data().cloned() {

                    // NOTE(Sam): It seems VERY odd that we don't drop individual stream parts for being too
                    // old. Why assemble something that might never finish and is known to be stale?

                    // Check if the proposal is outdated
                    if parts.height.as_u64() < lock.height {
                        info!(
                            height = %lock.height,
                            round = %lock.round,
                            part.height = %parts.height,
                            part.round = %parts.round,
                            part.sequence = %sequence,
                            "Received outdated proposal part, ignoring"
                        );
                    } else {

                        // signature verification
                        {
                            let mut hasher = sha3::Keccak256::new();

                            let hash = {
                                hasher.update(parts.height.as_u64().to_be_bytes());
                                hasher.update(parts.round.as_i64().to_be_bytes());
                                hasher.update(&parts.data_bytes);
                                hasher.finalize()
                            };

                            // TEMP get the proposers key
                            let mut proposer_public_key = None;
                            for peer in tfl_handle.config.malachite_peers.iter() {
                                let (_, _, public_key) =
                                    rng_private_public_key_from_address(&peer);
                                if public_key == parts.proposer {
                                    proposer_public_key = Some(public_key);
                                    break;
                                }
                            }

                            // Verify the signature
                            assert!(my_signing_provider.verify(&hash, &parts.signature, &proposer_public_key.expect("proposer not found")));
                        }

                        // Re-assemble the proposal from its parts
                        let value : MalProposedValue::<MalContext> = {
                            let value = MalValue::new(parts.data_bytes.zcash_deserialize_into::<BftPayload>().unwrap());

                            let new_final_hash = value.value.headers.first().expect("at least 1 header").hash();

                            let validity = if let Some(new_final_height) = block_height_from_hash(&tfl_handle.call, new_final_hash).await {
                                MalValidity::Valid
                            } else {
                                error!("Voting against proposal. I do not know of PoW hash {}", new_final_hash);
                                MalValidity::Invalid
                            };

                            MalProposedValue {
                                height: parts.height,
                                round: parts.round,
                                valid_round: parts.pol_round,
                                proposer: MalPublicKey2(parts.proposer),
                                value,
                                validity,
                            }
                        };


                        info!(
                            "Storing undecided proposal {} {}",
                            value.height, value.round
                        );

                        assert_eq!(value.height.as_u64(), lock.height);
                        assert_eq!(value.round.as_u32().unwrap(), lock.round);
                        while at_this_height_previously_seen_proposals.len() < lock.round as usize + 1 { at_this_height_previously_seen_proposals.push(None); }
                        at_this_height_previously_seen_proposals[lock.round as usize] = Some(Box::new(value.clone()));

                        if reply.send(Some(value)).is_err() {
                            error!("Failed to send ReceivedProposalPart reply");
                        }
                    }
                }
            },
            _ => panic!("AppMsg variant not handled: {:?}", app_msg),
        }
        break;
        }

        push_new_bft_msg_flags(&tfl_handle, bft_msg_flags).await;
    }
}
