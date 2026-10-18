#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p corpus && cd corpus
[ -d mdit-src ] || git clone -q --depth 1 https://github.com/markdown-it/markdown-it mdit-src
[ -d rust-book ] || git clone -q --depth 1 https://github.com/rust-lang/book rust-book
[ -f spec.md ] || curl -sfL https://raw.githubusercontent.com/commonmark/commonmark-spec/master/spec.txt -o spec.md
