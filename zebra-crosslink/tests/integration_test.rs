
#[cfg(test)]
mod integration_tests {
    use serde::Serialize;
    use std::sync::Arc;
    use zebra_chain::block::{Block, Hash as BlockHash, Height as BlockHeight};
    use zebra_chain::serialization::ZcashDeserialize;
    use zebra_crosslink::viz::VizState;
    use zebra_test::vectors::MAINNET_BLOCKS;

    fn get_test_block(height: u32) -> Arc<Block> {
        let bytes = MAINNET_BLOCKS.get(&height).unwrap();
        let block = Block::zcash_deserialize::<&[u8]>(bytes.as_ref()).expect("valid block");
        Arc::new(block)
    }

    #[test]
    fn crosslink_test_chain_growth_headless() {
        let blocks: Vec<Option<Arc<Block>>> = (0..5).map(|i| Some(get_test_block(i))).collect();
        let hashes: Vec<BlockHash> = blocks.iter().map(|b| b.as_ref().unwrap().hash()).collect();

        let state = Arc::new(VizState {
            latest_final_block: Some((BlockHeight(2), hashes[2])),
            bc_tip: Some((BlockHeight(4), hashes[4])),
            lo_height: BlockHeight(0),
            hashes: hashes.clone(),
            blocks: blocks.clone(),
            internal_proposed_bft_string: Some("Genesis".into()),
            bft_block_strings: vec!["A:0".into(), "B:1".into(), "C:".into()],
        });

        assert_eq!(blocks.len(), 5);
        assert_eq!(state.hashes.len(), 5);
        assert_eq!(state.lo_height, BlockHeight(0));

        #[cfg(feature = "viz_gui")]
        {
            use serde_json::to_string_pretty;
            use std::fs;

            let viz_export = MinimalVizStateExport::from(&*state);
            let json = to_string_pretty(&viz_export).expect("serialization success");
            fs::write("viz_state.json", json).expect("write success");

            eprintln!("🖼  Run `cargo run --bin viz_test_driver` to view visualization");
        }
    }

    #[derive(Serialize)]
    #[cfg(feature = "viz_gui")]
    struct MinimalBlockExport {
        difficulty: u32,
        txs_n: u32,
    }

    #[derive(Serialize)]
    #[cfg(feature = "viz_gui")]
    struct MinimalVizStateExport {
        latest_final_block: Option<(u32, [u8; 32])>,
        bc_tip: Option<(u32, [u8; 32])>,
        lo_height: u32,
        hashes: Vec<[u8; 32]>,
        blocks: Vec<Option<MinimalBlockExport>>,
        bft_block_strings: Vec<String>,
    }

    #[cfg(feature = "viz_gui")]
    impl From<&VizState> for MinimalVizStateExport {
        fn from(state: &VizState) -> Self {
            Self {
                latest_final_block: state.latest_final_block.map(|(h, hash)| (h.0, hash.0)),
                bc_tip: state.bc_tip.map(|(h, hash)| (h.0, hash.0)),
                lo_height: state.lo_height.0,
                hashes: state.hashes.iter().map(|h| h.0).collect(),
                blocks: state.blocks.iter().map(|opt| {
                    opt.as_ref().map(|b| MinimalBlockExport {
                        difficulty: u32::from_be_bytes(b.header.difficulty_threshold.bytes_in_display_order()),
                        txs_n: b.transactions.len() as u32,
                    })
                }).collect(),
                bft_block_strings: state.bft_block_strings.clone(),
            }
        }
    }
}
