<p align="center"><img src="logo/a.svg" width="160" alt="mdust"></p>

# mdust

mdust is a Markdown to HTML renderer written in Rust and compiled to WebAssembly. Its output is byte-identical to [markdown-it](https://github.com/markdown-it/markdown-it) 15.0.3 with preset `default` and `{ html: true }` on every document the repository tests. In Node it renders the 21 benchmark corpora 8.3 to 15.1 times faster than markdown-it, measured on 2026-10-19 with `node run.mjs` in `bench/`. The [npm package README](npm/README.md) has install, usage, the design notes and the full benchmark table.

## A fixed target

The repository started on 2026-10-18 in a folder named `challenge`. The first commit, `770f7db`, contains no parser. It times markdown-it against markdown-wasm, which is md4c compiled to wasm, and against the markdown-it Rust crate. On rust-book, md4c ran 5.1 times faster than markdown-it and the Rust crate 1.4 times.

The target never moved after that. It is the HTML that markdown-it 15.0.3 returns for the same input, compared with `===` in `bench/conformance.mjs`. Where the CommonMark spec and markdown-it disagree, which happens on 61 spec examples, mdust copies markdown-it.

## The checker came before the parser

The second commit, `06e5c53`, adds `bench/conformance.mjs` and a differential fuzzer, still with no parser to check. The harness also loads `control-broken`, a copy of markdown-it that swaps `&amp;` for `&#38;` and drops a `</p>` whenever a hash of the input is divisible by seven. That control has to fail every suite, or the suite proves nothing. Three parsers were then written from one brief and scored by the harness. The base picked on 2026-10-18 is `6d02d16`.

## Corpora we tune on, corpora we don't

Speed is judged on five headline corpora, listed in `bench/run.mjs` as rust-book, node-api, mdn-js, k8s-concepts and awesome. After a round of tuning the worry was overfitting, so main was timed on repositories nobody had looked at. The geomean dropped from 5.08x to 4.87x, a gap small enough to keep going. Six holdouts now live in `bench/corpora.mjs` under the comment "measured, never tuned against", and the PGO profile in `bench/pgo.sh` trains on rust-book and node-api only.

## How it got fast

Each row is a commit on main, timed against markdown-it in Node with `bench/run.mjs`, pinned to one core, median of 7 alternating runs.

```text
770f7db  2026-10-18  bench only, no parser           markdown-wasm 5.1x on rust-book
6d02d16  2026-10-18  arena base, opt-level "s"       mdust 4.30x on rust-book
e59658e  2026-10-18  eleven measured levers          mdust 5.08x headline geomean
a209207  2026-10-18  opt-level 3, simd128 scanners   mdust 8.36x headline geomean
7cef3b4  2026-10-19  bump allocator, two buffers     mdust 10.16x headline geomean
```

The crate is `#![no_std]` with one dependency, `hashbrown`. `render` parses the input in place. Only if the line scan sees a CR or a NUL does it normalize and parse a second time. Text and URLs that need no rewriting stay borrowed slices of the input until the renderer writes them out.

Three scanners use wasm `simd128` and read 16 bytes per step. `.cargo/config.toml` turns the feature on for `wasm32-unknown-unknown`. The first SIMD grafts were rejected. They measured no gain, or they pushed the wasm 11 KB over a 153,600 byte cap. Then the cap went, because it blocked simd128 at 162,899 bytes. The comment in `Cargo.toml` used to read "the size budget wins". It now reads "speed wins over size", and the wasm file is 179,356 bytes.

## Two buffers and a bump

The wasm module exports `mdust_in`, `mdust_render`, `mdust_out_len` and `memory`, and nothing else. The Node loader in `npm/mdust_wasm.js` is the whole boundary:

```js
function render(src) {
  src = String(src);
  // utf8Write never needs more than 3 bytes per UTF-16 unit.
  const cap = src.length * 3;
  const inPtr = wasm.mdust_in(cap);
  if (buf.buffer !== wasm.memory.buffer) buf = Buffer.from(wasm.memory.buffer);
  const len = buf.utf8Write(src, inPtr, cap);
  const outPtr = wasm.mdust_render(len);
  if (buf.buffer !== wasm.memory.buffer) buf = Buffer.from(wasm.memory.buffer);
  return buf.utf8Slice(outPtr, outPtr + wasm.mdust_out_len());
}
```

`mdust_in` resets the allocator in `crates/mdust-wasm/src/bump.rs` to the same base address on every call. Nothing is freed one block at a time, except that freeing the newest block hands its bytes back. When the arena runs out it grows by at least 16 wasm pages, and the next render starts from the base again.

## Native

The same crate builds for x86-64. `crates/mdust/src/x86.rs` reads `cpuid` once and picks SSE2, SSSE3 or AVX2 for the scanners, because a `no_std` crate cannot call `is_x86_feature_detected!`. The benchmark binary in `bench/native` adds mimalloc and the PGO build from `bench/pgo.sh`. `crates/mdust-napi` wraps `mdust::render` as a Node add-on.

On 2026-10-19, `RUNS=7 CPU=10 node run.mjs` gave these geomeans against markdown-it. mdust-rs in Node scored 10.14x headline, 9.85x holdout and 9.67x heavy. The native binary scored 15.09x, 13.87x and 14.16x.

## Repository layout

- `crates/mdust` is the parser and renderer, a `no_std` Rust library.
- `crates/mdust-wasm` exports `mdust::render` to WebAssembly through two buffers that the JS loaders read and write.
- `npm/` is the `mdust-rs` npm package. npm refused the bare name mdust as too close to an existing package. It holds the Node loader `mdust_wasm.js`, its ES module wrapper `mdust_wasm.mjs`, the browser loader `mdust_wasm.web.mjs`, and the types in `index.d.ts`.
- `bench/` holds the parity checks and the benchmark harness, and compares every output against markdown-it.
- `tools/gen_tables.cjs` generates `crates/mdust/src/tables.rs`, the entity and Unicode punctuation tables.
- `crates/mdust-napi` and `native/` build a native Node add-on, marked private in `native/package.json`.
- `logo/` holds three logo concepts as SVG.

## Build

You need a Rust toolchain with the `wasm32-unknown-unknown` target, and Node with npm.

1. To build the wasm file into `npm/`, run `npm --prefix npm run build` from the repository root.
2. To make the npm tarball, run `npm pack` in `npm/`. It writes `npm/mdust-rs-0.1.1.tgz`.

The build output, `npm/mdust_wasm_bg.wasm` and the tarball, is not committed.

## Check parity and benchmark

The benchmark harness reads its corpora from `bench/corpus`, which is not committed.

1. In `bench/`, run `npm install` and `./fetch-corpus.sh` to get markdown-it and the corpora.
2. Run `npm run build:mdust`. It builds `npm/` and copies the wasm file and the loaders into `crates/mdust-wasm/pkg`, which the harness loads.
3. To check parity against markdown-it on every fixture suite and corpus, run `node conformance.mjs --impl mdust`.
4. To time every implementation, run `node run.mjs`. `CORPORA=rust-book RUNS=1` limits a run to one corpus and one sample. `CPU=<n>` picks the core that `taskset` pins each run to.
5. To check the browser loader in headless Chromium, run `node browser.mjs`.

`run.mjs` also times markdown-wasm, the markdown-it.rs crate, and a native build of mdust. To build the markdown-it.rs and native binaries, run `npm run build` in `bench/`.
