//! Test-format-based tests

#![cfg(test)]
use zebrad::prelude::Application;

/// Unified codepath for test-format-based tests.
/// Starts full application with globals set ready for internal instrs thread.
pub fn test_file(path: &str) {
    // init globals
    {
        *zebra_crosslink::TEST_INSTR_PATH.lock().unwrap() = Some(path.into());
        *zebra_crosslink::TEST_SHUTDOWN_FN.lock().unwrap() = || APPLICATION.shutdown(abscissa_core::Shutdown::Graceful);
    }

    use zebrad::application::{
        APPLICATION,
        ZebradApp
    };

    // boot
    let os_args: Vec<_> = std::env::args_os().collect();
    // panic!("OS args: {:?}", os_args);
    let args: Vec<std::ffi::OsString> = vec![os_args[0].clone(), zebrad::commands::EntryPoint::default_cmd_as_str().into()];
    // println!("args: {:?}", args);
    ZebradApp::run(&APPLICATION, args);
}

#[test]
fn read_from_file() {
    test_file("../blocks.zeccltf");
}


#[test]
fn read_from_file2() {
    test_file("../blocks.zeccltf");
}
