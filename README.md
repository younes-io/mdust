# mdust

mdust is a Markdown to HTML renderer written in Rust and compiled to WebAssembly. Its output is byte-identical to [markdown-it](https://github.com/markdown-it/markdown-it) 15.0.3 with preset `default` and `{ html: true }` on every document the repository tests. In Node it renders the 21 benchmark corpora 8.3 to 15.1 times faster than markdown-it, measured on 2026-10-19 with `node run.mjs` in `bench/`. The [npm package README](npm/README.md) has install, usage, the design notes and the full benchmark table.

## Repository layout

- `crates/mdust` is the parser and renderer, a `no_std` Rust library.
- `crates/mdust-wasm` exports `mdust::render` to WebAssembly through two buffers that the JS loaders read and write.
- `npm/` is the `mdust-rs` npm package. npm refused the bare name mdust as too close to an existing package. It holds the Node loader `mdust_wasm.js`, its ES module wrapper `mdust_wasm.mjs`, the browser loader `mdust_wasm.web.mjs`, and the types in `index.d.ts`.
- `bench/` holds the parity checks and the benchmark harness, and compares every output against markdown-it.
- `tools/gen_tables.cjs` generates `crates/mdust/src/tables.rs`, the entity and Unicode punctuation tables.
- `crates/mdust-napi` and `native/` build a native Node add-on, marked private in `native/package.json`.

## Build

You need a Rust toolchain with the `wasm32-unknown-unknown` target, and Node with npm.

1. To build the wasm file into `npm/`, run this command from the repository root:

	```sh
	npm --prefix npm run build
	```

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
