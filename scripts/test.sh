#!/usr/bin/env sh
set -e
set -x #echo on

export RUST_BACKTRACE=full

cargo test $@

# uncomment if features are added to the project
# cargo test $@ --all-features
