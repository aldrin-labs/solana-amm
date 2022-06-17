#!/bin/bash

# Generates a code coverage report into target/debug/coverage.
#
# Source: https://github.com/mozilla/grcov

set -e

# by default generate html output, but also work with cobertura so that we can
# upload statistics on our gitlab
# https://docs.gitlab.com/ee/ci/yaml/artifacts_reports.html#artifactsreportscoverage_report
output_type=${1:-"html"}
output_path="./target/debug/coverage/"
mkdir -p "${output_path}"
if [ "${output_type}" == "cobertura" ]; then
    output_path="./target/debug/coverage/cobertura.xml"
fi

# dependencies install will be skipped if already present
cargo install grcov
rustup component add llvm-tools-preview

export RUSTC_BOOTSTRAP=1
export CARGO_INCREMENTAL=0
export RUSTFLAGS="-Cinstrument-coverage"
export LLVM_PROFILE_FILE="target/codecov/amm-%p-%m.profraw"

cargo test

mkdir -p target/codecov
log_file="target/codecov/grcov.log"

rm -f "${log_file}"
grcov . \
    -s . \
    --binary-path ./target/debug/ \
    -t "${output_type}" \
    --branch \
    --log "${log_file}" \
    --ignore-not-existing \
    -o "${output_path}"

head "${log_file}"
echo "..."
tail "${log_file}"
