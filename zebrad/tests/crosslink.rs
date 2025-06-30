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

#[test]
fn from_array() {
    let mut tf = TF::new(&PROTOTYPE_PARAMETERS);

    let regtest_block_bytes = [
        include_bytes!("../../crosslink-test-data/test_pow_block_0.bin"),
        include_bytes!("../../crosslink-test-data/test_pow_block_1.bin"),
        include_bytes!("../../crosslink-test-data/test_pow_block_2.bin"),
        include_bytes!("../../crosslink-test-data/test_pow_block_3.bin"),
        include_bytes!("../../crosslink-test-data/test_pow_block_4.bin"),
        include_bytes!("../../crosslink-test-data/test_pow_block_5.bin"),
        include_bytes!("../../crosslink-test-data/test_pow_block_6.bin"),
    ];

    for i in 0..regtest_block_bytes.len() {
        tf.push_instr(TFInstr::LOAD_POW, regtest_block_bytes[i]);
    }

    let bytes = tf.write_to_bytes();
    test_start(TestInstrSrc::Bytes(bytes));
}
