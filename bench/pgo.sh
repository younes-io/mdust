#!/usr/bin/env bash
# Profile-guided build of mdust-native, in two steps:
#   1. build with -Cprofile-generate (target/pgo-gen) and run the training set,
#      rust-book plus node-api, TRAIN_MS of timed passes per corpus;
#   2. merge the .profraw files with the toolchain's llvm-profdata
#      (rustup component add llvm-tools) and rebuild with -Cprofile-use
#      (target/pgo-use), then copy the result to
#      native/target/release/mdust-native-pgo, which run.mjs prefers when present.
# Finally prints the holdout check: PGO vs plain on mdn-js, k8s-concepts and
# awesome, alternating order, median of RUNS, pinned to CPU (ab.mjs).
# usage: RUNS=5 CPU=10 TRAIN_MS=2000 bench/pgo.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
nat=$here/native
tools=$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin
if [ ! -x "$tools/llvm-profdata" ]; then
  echo "missing $tools/llvm-profdata: run 'rustup component add llvm-tools'" >&2
  exit 1
fi
prof=$nat/target/pgo-profiles
rm -rf "$prof"
mkdir -p "$prof"

cargo build --release --manifest-path "$nat/Cargo.toml" --bin mdust-native
RUSTFLAGS="-Cprofile-generate=$prof" CARGO_TARGET_DIR=$nat/target/pgo-gen \
  cargo build --release --manifest-path "$nat/Cargo.toml" --bin mdust-native
for c in rust-book node-api; do
  files=$(node -e "import('$here/corpora.mjs').then((m) => console.log(m.corpora['$c']().join('\n')))")
  # shellcheck disable=SC2086
  "$nat/target/pgo-gen/release/mdust-native" "${TRAIN_MS:-2000}" $files >/dev/null
done
"$tools/llvm-profdata" merge -o "$prof/merged.profdata" "$prof"/*.profraw
RUSTFLAGS="-Cprofile-use=$prof/merged.profdata" CARGO_TARGET_DIR=$nat/target/pgo-use \
  cargo build --release --manifest-path "$nat/Cargo.toml" --bin mdust-native
cp "$nat/target/pgo-use/release/mdust-native" "$nat/target/release/mdust-native-pgo"
echo "wrote $nat/target/release/mdust-native-pgo"

echo "holdout: plain (A) vs pgo (B)"
RUNS=${RUNS:-5} CPU=${CPU:-10} CORPORA=mdn-js,k8s-concepts,awesome \
  node "$here/ab.mjs" "$nat/target/release/mdust-native" "$nat/target/release/mdust-native-pgo"
