//! Test-format-based tests

#![cfg(test)]
use zebrad::prelude::Application;
use zebra_crosslink::chain::*;
use zebra_crosslink::test_format::*;

pub fn test_start(src: TestInstrSrc) {
    // init globals
    {
        *zebra_crosslink::TEST_INSTR_SRC.lock().unwrap() = Some(src);
        *zebra_crosslink::TEST_SHUTDOWN_FN.lock().unwrap() =
            || APPLICATION.shutdown(abscissa_core::Shutdown::Graceful);
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
    ZebradApp::run(&APPLICATION, args);
}

#[test]
fn read_from_file() {
    test_start(TestInstrSrc::Path("../crosslink-test-data/blocks.zeccltf".into()));
}

const REGTEST_BLOCK_BYTES: [&[u8]; 7] = [
    include_bytes!("../../crosslink-test-data/test_pow_block_0.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_1.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_2.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_3.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_4.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_5.bin"),
    include_bytes!("../../crosslink-test-data/test_pow_block_6.bin"),
];


#[test]
fn from_array() {
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
    }

    let bytes = tf.write_to_bytes();
    test_start(TestInstrSrc::Bytes(bytes));
}

#[test]
fn from_array2() {
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
    }

    let bad_block = [1u8; 128];
    tf.push_instr(TFInstr::LOAD_POW, &bad_block);

    let bytes = tf.write_to_bytes();
    test_start(TestInstrSrc::Bytes(bytes));
}


#[test]
fn from_array3() {
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    for i in 0..REGTEST_BLOCK_BYTES.len() {
        tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[i]);
    }

    tf.push_instr(TFInstr::LOAD_POW, REGTEST_BLOCK_BYTES[1]);

    let bytes = tf.write_to_bytes();
    test_start(TestInstrSrc::Bytes(bytes));
}
