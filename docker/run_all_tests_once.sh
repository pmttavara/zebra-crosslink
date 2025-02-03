#!/bin/sh

if [[ -z "$1" ]]; then
    RED='\033[0;31m'
    NC='\033[0m'
    echo -e "${RED}Error!${NC} You must give the path to a zebra cache as the first argument."
    echo "    e.g.  docker/run_all_tests_once.sh /home/sam/.cache/zebra/"
    echo ""
    echo "Zebra cache is 250 GiB and will take days to sync organically. I have created a"
    echo "zip file that you can download instead. http://70.34.198.4/zebra_cache.zip"
    exit 1
fi

ZC=$(realpath $1)

cd $(dirname $0)/..

export ZEBRA_CACHED_STATE_DIR=$ZC
export TEST_LWD_UPDATE_SYNC=
export RUN_ALL_TESTS=1
export TEST_LWD_GRPC=
export SHORT_SHA=
export LOG_COLOR=false
export TEST_ZEBRA_EMPTY_SYNC=
export RUST_VERSION=1.82.0
export FULL_SYNC_MAINNET_TIMEOUT_MINUTES=
export TEST_LWD_RPC_CALL=
export RUST_BACKTRACE=1
export RUST_LOG=info
export FULL_SYNC_TESTNET_TIMEOUT_MINUTES=

set -e
set -o pipefail

if [[ "$2" ]]; then
    exec cargo test $2 --locked --release --features "default-release-binaries lightwalletd-grpc-tests zebra-checkpoints" --workspace -- --nocapture --include-ignored
fi

cargo test --locked --release --features "default-release-binaries lightwalletd-grpc-tests zebra-checkpoints" --workspace -- --nocapture --include-ignored \
--skip config_tests \
--skip generate_checkpoints_mainnet \
--skip generate_checkpoints_testnet \
--skip zebrad_update_sync \
--skip lightwalletd_test_suite \
--skip get_block_template \
--skip submit_block \
--skip fully_synced_rpc_z_getsubtreesbyindex_snapshot_test

cargo test config_tests --locked --release --features "default-release-binaries lightwalletd-grpc-tests zebra-checkpoints" --workspace -- --nocapture --include-ignored


# TODO(Sam): fix and re-enable these tests
#    generate_checkpoints_mainnet
#    generate_checkpoints_testnet
#
# for these, the error is that zebra is too old:
#    zebrad_update_sync
#    lightwalletd_test_suite
#    get_block_template
#    submit_block
#    fully_synced_rpc_z_getsubtreesbyindex_snapshot_test
