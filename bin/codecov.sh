#!/bin/bash

# Generates a code coverage report into target/release/coverage. We use
# release to enable both cargo test, rust analyzer and code coverage runs at
# the same time without the need to recompile.
#
# Source: https://github.com/mozilla/grcov

set -e

# by default generate html output, but also work with cobertura so that we can
# upload statistics on our gitlab
# https://docs.gitlab.com/ee/ci/yaml/artifacts_reports.html#artifactsreportscoverage_report
output_type=${1:-"html"}
output_path="./target/release/coverage/"
mkdir -p "${output_path}"
if [ "${output_type}" == "cobertura" ]; then
    output_path="./target/release/coverage/cobertura.xml"
fi

# dependencies install will be skipped if already present
cargo install grcov
rustup component add llvm-tools-preview

export RUSTC_BOOTSTRAP=1
export CARGO_INCREMENTAL=0
export RUSTFLAGS="-Cinstrument-coverage"
export LLVM_PROFILE_FILE="target/codecov/amm-%p-%m.profraw"

cargo test --release

mkdir -p target/codecov
log_file="target/codecov/grcov.log"

rm -f "${log_file}"
grcov . \
    -s . \
    --binary-path ./target/release/ \
    -t "${output_type}" \
    --branch \
    --log "${log_file}" \
    --ignore-not-existing \
    -o "${output_path}"

head "${log_file}"
echo "..."
tail "${log_file}"
