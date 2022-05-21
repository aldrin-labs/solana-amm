#!/bin/bash

# Install following dependencies:
#
# cargo install grcov
# rustup component add llvm-tools-preview
#
# Source: https://github.com/mozilla/grcov

export RUSTC_BOOTSTRAP=1
export CARGO_INCREMENTAL=0
export RUSTFLAGS="-Cinstrument-coverage"
export LLVM_PROFILE_FILE="target/codecov/amm-%p-%m.profraw"

cargo build --lib
cargo test --lib

mkdir -p target/codecov
log_file="target/codecov/grcov.log"

rm -f "${log_file}"
grcov . -s . --binary-path ./target/debug/ -t html --branch --log "${log_file}" --ignore-not-existing -o ./target/debug/coverage/

head "${log_file}"
echo "..."
tail "${log_file}"
