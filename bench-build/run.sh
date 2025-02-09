#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

mkdir -p target
hyperfine \
    --runs=3 \
    --warmup=1 \
    --prepare='rm -r target' \
    'cargo run --features=derive' \
    'cargo run'
echo
echo ----------
echo
hyperfine \
    --runs=3 \
    --warmup=1 \
    --prepare='rm -r target' \
    'cargo run --release --features=derive' \
    'cargo run --release'
