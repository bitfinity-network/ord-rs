#!/usr/bin/env sh
set -e
set -x #echo on

export RUST_BACKTRACE=full

# before testing, the build.sh script should be executed
cargo test $@

# uncomment if features are added to the project
# cargo test $@ --all-features
