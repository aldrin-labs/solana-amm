#!/bin/bash

# pin solana
SOLANA_VERSION="1.9.18"
solana --version 2>&1 1>/dev/null || sh -c "$(curl -sSfL https://release.solana.com/${SOLANA_VERSION}/install)"
solana --version | grep "${SOLANA_VERSION}" || solana-install init "${SOLANA_VERSION}"

if [ -f .env ]
then
    export $(cat .env | sed 's/#.*//g' | xargs)
fi

detach=false
skip_build=false

while :; do
    case $1 in
        -d|--detach) detach=true
        ;;
        --skip-build) skip_build=true
        ;;
        *) break
    esac
    shift
done

echo "Running tests..."
echo

skip_build_flag=$($skip_build && echo "--skip-build")
detach_flag=$($detach && echo "--detach")
npm t -- ${skip_build_flag} ${detach_flag} -- --features dev
