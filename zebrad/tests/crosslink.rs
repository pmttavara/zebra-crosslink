//! Test-format-based tests

#![cfg(test)]

use std::time::Duration;

use zebrad::prelude::Application;
use zebra_crosslink::chain::*;
use zebra_crosslink::test_format::*;
use zebra_chain::serialization::*;

pub fn test_start(src: TestInstrSrc) {
    // init globals
    {
        *zebra_crosslink::TEST_INSTR_SRC.lock().unwrap() = Some(src);
        *zebra_crosslink::TEST_SHUTDOWN_FN.lock().unwrap() =
            || {
                // APPLICATION.shutdown(abscissa_core::Shutdown::Graceful);
                std::process::exit(0);
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

#[ignore]
#[test]
fn read_from_file() {
    test_start(TestInstrSrc::Path("../crosslink-test-data/blocks.zeccltf".into()));
}

const REGTEST_BLOCK_BYTES: [&[u8]; 6] = [
    include_bytes!("../../crosslink-test-data/test_pow_block_0.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_1.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_2.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_4.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_5.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_6.bin"),
    // include_bytes!("../../crosslink-test-data/test_pow_block_3.bin"),
];

const REGTEST_POS_BLOCK_BYTES: [&[u8]; 1] = [
    include_bytes!("../../crosslink-test-data/test_pos_block_5.bin"),
];


#[test]
fn crosslink_expect_pos_height_on_boot() {
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    tf.push_instr_val(TFInstr::EXPECT_POS_CHAIN_LENGTH, [0, 0]);

    let bytes = tf.write_to_bytes();
    test_start(TestInstrSrc::Bytes(bytes));
}

fn crosslink_expect_pow_height_on_boot() {
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    tf.push_instr_val(TFInstr::EXPECT_POW_CHAIN_LENGTH, [1, 0]);

    let bytes = tf.write_to_bytes();
    test_start(TestInstrSrc::Bytes(bytes));
}

#[test]
fn crosslink_expect_first_pow_to_not_be_a_no_op() {
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[0]);
    tf.push_instr_val(TFInstr::EXPECT_POW_CHAIN_LENGTH, [2 as u64, 0]);

    let bytes = tf.write_to_bytes();
    test_start(TestInstrSrc::Bytes(bytes));
}

#[test]
fn crosslink_push_example_pow_chain_only() {
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
        tf.push_instr_val(TFInstr::EXPECT_POW_CHAIN_LENGTH, [2+i as u64, 0]);
    }
    tf.push_instr_val(TFInstr::EXPECT_POW_CHAIN_LENGTH, [1+REGTEST_BLOCK_BYTES.len() as u64, 0]);

    let bytes = tf.write_to_bytes();
    test_start(TestInstrSrc::Bytes(bytes));
}

#[test]
fn crosslink_push_example_pow_chain_each_block_twice() {
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
        tf.push_instr_val(TFInstr::EXPECT_POW_CHAIN_LENGTH, [2+i as u64, 0]);
    }
    tf.push_instr_val(TFInstr::EXPECT_POW_CHAIN_LENGTH, [1+REGTEST_BLOCK_BYTES.len() as u64, 0]);

    let bytes = tf.write_to_bytes();
    test_start(TestInstrSrc::Bytes(bytes));
}

#[test]
fn crosslink_push_example_pow_chain_again_should_not_change_the_pow_chain_length() {
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
        tf.push_instr_val(TFInstr::EXPECT_POW_CHAIN_LENGTH, [2+i as u64, 0]);
    }
    tf.push_instr_val(TFInstr::EXPECT_POW_CHAIN_LENGTH, [1+REGTEST_BLOCK_BYTES.len() as u64, 0]);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
        tf.push_instr_val(TFInstr::EXPECT_POW_CHAIN_LENGTH, [1+REGTEST_BLOCK_BYTES.len() as u64, 0]);
    }

    let bytes = tf.write_to_bytes();
    test_start(TestInstrSrc::Bytes(bytes));
}

#[test]
fn crosslink_expect_pos_not_pushed_if_pow_blocks_not_present() {
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    tf.push_instr(TFInstr::LOAD_POS, REGTEST_POS_BLOCK_BYTES[0]);
    tf.push_instr_val(TFInstr::EXPECT_POS_CHAIN_LENGTH, [0, 0]);

    let bytes = tf.write_to_bytes();
    test_start(TestInstrSrc::Bytes(bytes));
}

#[test]
fn crosslink_expect_pos_height_after_push() {
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
    }
    tf.push_instr(TFInstr::LOAD_POS, REGTEST_POS_BLOCK_BYTES[0]);
    tf.push_instr_val(TFInstr::EXPECT_POS_CHAIN_LENGTH, [1, 0]);

    let bytes = tf.write_to_bytes();
    test_start(TestInstrSrc::Bytes(bytes));
}

#[test]
fn crosslink_expect_pos_push_same_block_twice_only_accepted_once() {
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
    }
    tf.push_instr(TFInstr::LOAD_POS, REGTEST_POS_BLOCK_BYTES[0]);
    tf.push_instr(TFInstr::LOAD_POS, REGTEST_POS_BLOCK_BYTES[0]);
    tf.push_instr_val(TFInstr::EXPECT_POS_CHAIN_LENGTH, [1, 0]);

    let bytes = tf.write_to_bytes();
    test_start(TestInstrSrc::Bytes(bytes));
}

#[test]
fn crosslink_reject_pos_with_signature_on_different_data() {
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
    }

    // modify block data so that the signatures are incorrect
    // NOTE: modifying the last as there is no `previous_block_hash` that this invalidates
    let mut bft_block_and_fat_ptr = BftBlockAndFatPointerToIt::zcash_deserialize(REGTEST_POS_BLOCK_BYTES[0]).unwrap();
    // let mut last_pow_hdr =
    bft_block_and_fat_ptr.block.headers.last_mut().unwrap().time += Duration::from_secs(1);
    let new_bytes = bft_block_and_fat_ptr.zcash_serialize_to_vec().unwrap();

    assert!(&new_bytes != REGTEST_POS_BLOCK_BYTES[0], "test invalidated if the serialization has not been changed");

    tf.push_instr(TFInstr::LOAD_POS, &new_bytes);
    tf.push_instr_val(TFInstr::EXPECT_POS_CHAIN_LENGTH, [0, 0]);

    let bytes = tf.write_to_bytes();
    test_start(TestInstrSrc::Bytes(bytes));
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
