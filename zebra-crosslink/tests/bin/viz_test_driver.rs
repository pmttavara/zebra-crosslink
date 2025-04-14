//! The visualizer needs to be on the main thread, so we need to run it in a separate process.

fn main() {
    use std::fs;
    use std::sync::Arc;

    use chrono::Utc;

    use zebra_chain::{
        block::merkle::Root,
        block::{Block, Header as BlockHeader, Hash as BlockHash, Height as BlockHeight},
        fmt::HexDebug,
        work::{difficulty::CompactDifficulty, equihash::Solution},
    };

    use serde::Deserialize;

    use zebra_crosslink::viz::{VizGlobals, VizState, VIZ_G};

    #[derive(Deserialize)]
    struct MinimalBlockExport {
        difficulty: u32,
        txs_n: u32,
    }

    #[derive(Deserialize)]
    struct MinimalVizStateExport {
        latest_final_block: Option<(u32, [u8; 32])>,
        bc_tip: Option<(u32, [u8; 32])>,
        lo_height: u32,
        hashes: Vec<[u8; 32]>,
        blocks: Vec<Option<MinimalBlockExport>>,
        bft_block_strings: Vec<String>,
    }

    impl From<MinimalVizStateExport> for VizGlobals {
        fn from(export: MinimalVizStateExport) -> Self {
            let state = Arc::new(VizState {
                latest_final_block: export
                    .latest_final_block
                    .map(|(h, hash)| (BlockHeight(h), BlockHash(hash))),
                bc_tip: export
                    .bc_tip
                    .map(|(h, hash)| (BlockHeight(h), BlockHash(hash))),
                lo_height: BlockHeight(export.lo_height),
                hashes: export.hashes.into_iter().map(BlockHash).collect(),
                blocks: export.blocks.into_iter().map(|opt| {
                    opt.map(|b| {
                        Arc::new(Block {
                            header: Arc::new(BlockHeader {
                                version: 0,
                                previous_block_hash: BlockHash([0; 32]),
                                merkle_root: Root::from_bytes_in_display_order(&[0u8; 32]),
                                commitment_bytes: HexDebug([0u8; 32]),
                                time: Utc::now(),
                                difficulty_threshold:
                                    CompactDifficulty::from_bytes_in_display_order(
                                        &b.difficulty.to_be_bytes(),
                                    )
                                    .expect("valid difficulty"),
                                nonce: HexDebug([0u8; 32]),
                                solution: Solution::for_proposal(),
                            }),
                            transactions: vec![].into(),
                        })
                    })
                }).collect(),
                internal_proposed_bft_string: Some("From Export".into()),
                bft_block_strings: export.bft_block_strings,
            });

            VizGlobals {
                state,
                bc_req_h: (0, 0),
                consumed: true,
                proposed_bft_string: None,
            }
        }
    }

    let raw = fs::read_to_string("./zebra-crosslink/viz_state.json").expect("viz_state.json exists");
    let export: MinimalVizStateExport = serde_json::from_str(&raw).expect("valid export JSON");
    let globals: VizGlobals = export.into();
    *VIZ_G.lock().unwrap() = Some(globals);

    let _ = zebra_crosslink::viz::main(None);
}
