#!/bin/bash

# Generates a code coverage report into target/debug/coverage.
#
# Source: https://github.com/mozilla/grcov

# dependencies install will be skipped if already present
cargo install grcov
rustup component add llvm-tools-preview

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
