//! Test-format-based tests

#![cfg(test)]

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use zebra_chain::parameters::testnet::ConfiguredActivationHeights;
use zebra_chain::parameters::Network;
use zebra_chain::serialization::*;
use zebra_crosslink::chain::*;
use zebra_crosslink::test_format::*;
use zebrad::application::CROSSLINK_TEST_CONFIG_OVERRIDE;
use zebrad::config::ZebradConfig;
use zebrad::prelude::Application;

macro_rules! function_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        name.strip_suffix("::f")
            .unwrap()
            .split("::")
            .last()
            .unwrap()
    }};
}

pub fn set_test_name(name: &'static str) {
    *zebra_crosslink::TEST_NAME.lock().unwrap() = name;
}

pub fn test_start() {
    // init globals
    {
        *CROSSLINK_TEST_CONFIG_OVERRIDE.lock().unwrap() = {
            let mut base = ZebradConfig::default();
            base.network.network = Network::new_regtest(ConfiguredActivationHeights::default());
            base.state.ephemeral = true;

            Some(std::sync::Arc::new(base))
        };
        *zebra_crosslink::TEST_MODE.lock().unwrap() = true;
        *zebra_crosslink::TEST_SHUTDOWN_FN.lock().unwrap() = || {
            // APPLICATION.shutdown(abscissa_core::Shutdown::Graceful);
            std::process::exit(*zebra_crosslink::TEST_FAILED.lock().unwrap());
        }
    }

    use zebrad::application::{ZebradApp, APPLICATION};

    // boot
    let os_args: Vec<_> = std::env::args_os().collect();
    // panic!("OS args: {:?}", os_args);
    let args: Vec<std::ffi::OsString> = vec![
        os_args[0].clone(),
        zebrad::commands::EntryPoint::default_cmd_as_str().into(),
    ];
    // println!("args: {:?}", args);

    #[cfg(feature = "viz_gui")]
    {
        // TODO: gate behind feature-flag
        // TODO: only open the visualization window for the `start` command.
        // i.e.: can we move it to that code without major refactor to make compiler happy?
        let tokio_root_thread_handle = std::thread::spawn(move || {
            ZebradApp::run(&APPLICATION, args);
        });

        zebra_crosslink::viz::main(Some(tokio_root_thread_handle));
    }

    #[cfg(not(feature = "viz_gui"))]
    ZebradApp::run(&APPLICATION, args);
}

pub fn test_bytes(bytes: Vec<u8>) {
    *zebra_crosslink::TEST_INSTR_BYTES.lock().unwrap() = bytes;
    test_start();
}

pub fn test_path(path: PathBuf) {
    *zebra_crosslink::TEST_INSTR_PATH.lock().unwrap() = Some(path);
    test_start();
}

#[ignore]
#[test]
fn read_from_file() {
    test_path("../crosslink-test-data/blocks.zeccltf".into());
}

const REGTEST_BLOCK_BYTES: &[&[u8]] = &[
    include_bytes!("../../crosslink-test-data/test_pow_block_0.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_1.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_2.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_3.bin"), // fork
    include_bytes!("../../crosslink-test-data/test_pow_block_4.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_6.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_7.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_9.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_10.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_12.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_13.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_14.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_15.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_17.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_18.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_19.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_21.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_22.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_23.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_25.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_26.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_28.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_29.bin"),
];

const REGTEST_POS_BLOCK_BYTES: &[&[u8]] = &[
    include_bytes!("../../crosslink-test-data/test_pos_block_5.bin"),
    include_bytes!("../../crosslink-test-data/test_pos_block_8.bin"),
    include_bytes!("../../crosslink-test-data/test_pos_block_11.bin"),
    include_bytes!("../../crosslink-test-data/test_pos_block_16.bin"),
    include_bytes!("../../crosslink-test-data/test_pos_block_20.bin"),
    include_bytes!("../../crosslink-test-data/test_pos_block_24.bin"),
    include_bytes!("../../crosslink-test-data/test_pos_block_27.bin"),
];

#[test]
fn crosslink_expect_pos_height_on_boot() {
    set_test_name(function_name!());
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    tf.push_instr_val(TFInstr::EXPECT_POS_CHAIN_LENGTH, [0, 0]);

    test_bytes(tf.write_to_bytes());
}

#[test]
fn crosslink_expect_pow_height_on_boot() {
    set_test_name(function_name!());
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    tf.push_instr_val(TFInstr::EXPECT_POW_CHAIN_LENGTH, [1, 0]);

    test_bytes(tf.write_to_bytes());
}

#[test]
fn crosslink_expect_first_pow_to_not_be_a_no_op() {
    set_test_name(function_name!());
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[0]);
    tf.push_instr_val(TFInstr::EXPECT_POW_CHAIN_LENGTH, [2_u64, 0]);

    test_bytes(tf.write_to_bytes());
}

#[test]
fn crosslink_push_example_pow_chain_only() {
    set_test_name(function_name!());
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
        tf.push_instr_val(
            TFInstr::EXPECT_POW_CHAIN_LENGTH,
            [(2 + i - (i >= 3) as usize) as u64, 0],
        );
    }
    tf.push_instr_val(
        TFInstr::EXPECT_POW_CHAIN_LENGTH,
        [1 - 1 + REGTEST_BLOCK_BYTES.len() as u64, 0],
    );

    test_bytes(tf.write_to_bytes());
}

#[test]
fn crosslink_push_example_pow_chain_each_block_twice() {
    set_test_name(function_name!());
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
        tf.push_instr_val(
            TFInstr::EXPECT_POW_CHAIN_LENGTH,
            [(2 + i - (i >= 3) as usize) as u64, 0],
        );
    }
    tf.push_instr_val(
        TFInstr::EXPECT_POW_CHAIN_LENGTH,
        [1 - 1 + REGTEST_BLOCK_BYTES.len() as u64, 0],
    );

    test_bytes(tf.write_to_bytes());
}

#[test]
fn crosslink_push_example_pow_chain_again_should_not_change_the_pow_chain_length() {
    set_test_name(function_name!());
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
        tf.push_instr_val(
            TFInstr::EXPECT_POW_CHAIN_LENGTH,
            [(2 + i - (i >= 3) as usize) as u64, 0],
        );
    }
    tf.push_instr_val(
        TFInstr::EXPECT_POW_CHAIN_LENGTH,
        [1 - 1 + REGTEST_BLOCK_BYTES.len() as u64, 0],
    );

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
        tf.push_instr_val(
            TFInstr::EXPECT_POW_CHAIN_LENGTH,
            [1 - 1 + REGTEST_BLOCK_BYTES.len() as u64, 0],
        );
    }

    test_bytes(tf.write_to_bytes());
}

#[test]
fn crosslink_expect_pos_not_pushed_if_pow_blocks_not_present() {
    set_test_name(function_name!());
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    tf.push_instr(TFInstr::LOAD_POS, REGTEST_POS_BLOCK_BYTES[0]);
    tf.push_instr_val(TFInstr::EXPECT_POS_CHAIN_LENGTH, [0, 0]);

    test_bytes(tf.write_to_bytes());
}

#[test]
fn crosslink_expect_pos_height_after_push() {
    set_test_name(function_name!());
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
    }
    for i in 0..REGTEST_POS_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POS, REGTEST_POS_BLOCK_BYTES[i]);
        tf.push_instr_val(TFInstr::EXPECT_POS_CHAIN_LENGTH, [(1 + i) as u64, 0]);
    }

    // let write_ok = tf.write_to_file(Path::new("crosslink_expect_pos_height_after_push.zeccltf"));
    // assert!(write_ok);
    test_bytes(tf.write_to_bytes());
}

#[test]
fn crosslink_expect_pos_out_of_order() {
    set_test_name(function_name!());
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..5 {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
    }

    tf.push_instr(TFInstr::LOAD_POS, REGTEST_POS_BLOCK_BYTES[0]);
    tf.push_instr(TFInstr::LOAD_POS, REGTEST_POS_BLOCK_BYTES[2]);
    tf.push_instr(TFInstr::LOAD_POS, REGTEST_POS_BLOCK_BYTES[1]);
    tf.push_instr_val(TFInstr::EXPECT_POS_CHAIN_LENGTH, [2, 0]);

    test_bytes(tf.write_to_bytes());
}

#[test]
fn crosslink_expect_pos_push_same_block_twice_only_accepted_once() {
    set_test_name(function_name!());
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
    }
    tf.push_instr(TFInstr::LOAD_POS, REGTEST_POS_BLOCK_BYTES[0]);
    tf.push_instr(TFInstr::LOAD_POS, REGTEST_POS_BLOCK_BYTES[0]);
    tf.push_instr_val(TFInstr::EXPECT_POS_CHAIN_LENGTH, [1, 0]);

    test_bytes(tf.write_to_bytes());
}

#[test]
fn crosslink_reject_pos_with_signature_on_different_data() {
    set_test_name(function_name!());
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
    }

    // modify block data so that the signatures are incorrect
    // NOTE: modifying the last as there is no `previous_block_hash` that this invalidates
    let mut bft_block_and_fat_ptr =
        BftBlockAndFatPointerToIt::zcash_deserialize(REGTEST_POS_BLOCK_BYTES[0]).unwrap();
    // let mut last_pow_hdr =
    bft_block_and_fat_ptr.block.headers.last_mut().unwrap().time += Duration::from_secs(1);
    let new_bytes = bft_block_and_fat_ptr.zcash_serialize_to_vec().unwrap();

    assert!(
        &new_bytes != REGTEST_POS_BLOCK_BYTES[0],
        "test invalidated if the serialization has not been changed"
    );

    tf.push_instr(TFInstr::LOAD_POS, &new_bytes);
    tf.push_instr_val(TFInstr::EXPECT_POS_CHAIN_LENGTH, [0, 0]);

    test_bytes(tf.write_to_bytes());
}

// TODO:
// - reject signatures from outside the roster
// - reject pos block with < 2/3rds roster stake
// - reject pos block with signatures from the previous, but not current roster
// > require correctly-signed incorrect data:
//   - reject pos block with > sigma headers
//   - reject pos block with < sigma headers
//   - reject pos block where headers don't form subchain (hdrs[i].hash() != hdrs[i+1].previous_block_hash)
//   - repeat all signature tests but for the *next* pos block's fat pointer back
// - reject pos block that does have the correct fat pointer *hash* to prev block
// ...
