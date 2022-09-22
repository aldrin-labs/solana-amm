#!/bin/sh

# Runs linters

set -e

cargo fmt --check
cargo clippy -- -D warnings

yarn

npm run fmt-check
npm run lint

# we need to build idl to check TS compiles
npm run build
npm run ts-check
