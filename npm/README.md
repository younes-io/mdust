# mdust-rs

mdust is a Markdown to HTML renderer written in Rust as a `no_std` crate and compiled to WebAssembly. `render(src)` returns the same bytes as markdown-it 15.0.3 with preset `default` and `{ html: true }` on all 40,919 documents and fixture cases the repository tests. On the 21 benchmark corpora below, measured on 2026-10-19, it runs 8.3 to 15.1 times faster than markdown-it in Node. The package is called `mdust-rs` because npm refused the name `mdust` as too close to an existing package.

## Install

```sh
npm install mdust-rs
```

The package has no dependencies. It holds one wasm file, three loaders of 77 to 1,675 bytes, and a type file.

## Use it in Node

The Node entry loads the wasm module synchronously on import. You can call `render` right away.

```js
const { render } = require('mdust-rs');   // CommonJS
import { render } from 'mdust-rs';        // ES modules

render('# hi'); // '<h1>hi</h1>\n'
```

## Use it in the browser

The browser entry fetches and compiles the wasm module asynchronously. Call `await init()` once before the first `render`. The Node entries also export `init`, which resolves at once, so code shared between the two can always call it.

With no argument, `init` fetches `mdust_wasm_bg.wasm` from the directory of `mdust_wasm.web.mjs`:

```html
<script type="module">
	import { init, render } from './node_modules/mdust-rs/mdust_wasm.web.mjs';

	await init();
	document.body.innerHTML = render('# hi');
</script>
```

If the wasm file lives somewhere else, pass its location, as in `await init(new URL('/assets/mdust_wasm_bg.wasm', import.meta.url))`. `init` also accepts a `Response`, a promise of a `Response`, a compiled `WebAssembly.Module`, or the bytes as an `ArrayBuffer` or typed array. If the server sends `Content-Type: application/wasm`, `init` compiles the file while it downloads. Otherwise it downloads the whole file first.

Bundlers pick the browser entry through the `browser` and `default` export conditions. Vite and webpack 5 rewrite `new URL('file', import.meta.url)` and copy the wasm file into the build. With other bundlers, copy `mdust-rs/mdust_wasm_bg.wasm` into your static assets and pass its URL to `init`. This repository does not test these bundler setups.

## What is inside

- The crate has one dependency, `hashbrown` with default features off, for the link reference map.
- Three scanners use wasm `simd128`, 16 bytes per step. One finds line ends and flags any CR or NUL. One finds the bytes where a run of inline text ends, with a two-nibble `i8x16_swizzle` lookup. One finds the four bytes HTML escaping must replace. The native build picks SSE2, SSSE3 or AVX2 at run time through `cpuid`.
- `render` parses the input as it is. Only if the line scan saw a CR or NUL does it normalize those the way markdown-it does and parse again, so most inputs are parsed in place with no copy.
- Text that needs no unescaping, and URLs that are already normalized, stay borrowed slices of the input until the renderer writes them out.
- The wasm module has a bump allocator. Each render resets it to the same base address, and it gives memory back only for the most recent block.
- Strings cross into wasm through two buffers that live across calls. The loader encodes the input straight into wasm memory, with `Buffer.utf8Write` in Node and `TextEncoder.encodeInto` in the browser, and decodes the output from it. Apart from the result string, a render allocates on the JS side only after wasm memory has grown.
- The 2,125 named HTML entities and the 355 Unicode punctuation ranges are tables that `tools/gen_tables.cjs` generates with the `entities` package and markdown-it's own `isPunctCharCode`.
- The release profile uses `lto = true`, `codegen-units = 1`, `panic = "abort"` and `opt-level = 3`. A comment in `Cargo.toml` records that `opt-level = 3` buys about 5% throughput for about 36 KB of wasm, and the build takes the speed. The wasm file the benchmark ran on 2026-10-19 measured 179,356 bytes.

## Parity with markdown-it

`render(src)` has no options. Its target is the string that `new MarkdownIt('default', { html: true }).render(src)` returns. If you need other options or plugins, use markdown-it.

The repository checks this with `node conformance.mjs --impl mdust` in `bench/`, which compares every output to markdown-it's with `===`. On 2026-10-19 it passed 100% of 27 suites with no failures. The suites are the 652 CommonMark spec examples, the markdown-it fixture files for tables, strikethrough and CommonMark extras, and every document of 23 corpora, 40,141 documents in all. For 61 of the spec examples, markdown-it's output differs from the HTML the spec file expects, and mdust matches markdown-it.

## Benchmarks

| set | corpus | docs | MiB | markdown-it ms | markdown-wasm | markdown-it.rs wasm | mdust-rs | mdust native |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| headline | rust-book | 112 | 1.2 | 28.2 | 5.20x | 1.39x | 8.40x | 13.12x |
| headline | node-api | 70 | 4.5 | 136.3 | 4.99x | 1.36x | 8.84x | 13.02x |
| headline | mdn-js | 1308 | 5.4 | 163.8 | 5.49x | 1.51x | 9.78x | 14.51x |
| headline | k8s-concepts | 187 | 2.2 | 65.1 | 6.03x | 1.49x | 9.77x | 13.96x |
| headline | awesome | 6 | 1.3 | 75.8 | 9.48x | 2.54x | 15.08x | 22.64x |
| headline | geomean | | | | 6.06x | 1.61x | 10.14x | 15.09x |
| holdout | github-docs | 1200 | 6.6 | 200.1 | 6.46x | 1.46x | 10.88x | 15.93x |
| holdout | vue-docs | 119 | 0.98 | 25.2 | 5.05x | 1.52x | 9.27x | 12.49x |
| holdout | rust-rfcs | 643 | 8.8 | 252.7 | 5.56x | 1.47x | 9.35x | 13.39x |
| holdout | tldr | 6800 | 3.8 | 196.5 | 6.61x | 1.59x | 9.66x | 12.62x |
| holdout | electron-docs | 302 | 1.9 | 63.3 | 5.58x | 1.52x | 9.28x | 13.27x |
| holdout | obsidian-help | 179 | 0.72 | 31.5 | 6.61x | 1.73x | 10.84x | 15.95x |
| holdout | geomean | | | | 5.95x | 1.55x | 9.85x | 13.87x |
| heavy | github-docs-full | 3760 | 19.6 | 607.5 | 6.37x | 1.50x | 10.68x | 15.85x |
| heavy | mdn-web | 11005 | 39.7 | 1202.5 | 5.65x | 1.48x | 10.10x | 14.73x |
| heavy | k8s-docs | 1722 | 15.5 | 244.7 | 4.65x | 1.35x | 9.25x | 13.37x |
| heavy | vscode-docs | 364 | 4.8 | 148.5 | 6.09x | 1.58x | 9.97x | 14.40x |
| heavy | docker-docs | 1095 | 7.9 | 267.1 | 6.12x | 1.65x | 10.40x | 15.07x |
| heavy | fcc-curriculum | 9000 | 25.1 | 497.1 | 4.29x | 1.36x | 9.25x | 13.53x |
| heavy | eips | 966 | 6.5 | 166.0 | 5.81x | 1.57x | 10.08x | 14.76x |
| heavy | react-dev | 225 | 3.8 | 82.2 | 4.41x | 1.47x | 8.34x | 12.20x |
| heavy | ydkjs | 55 | 0.88 | 19.5 | 5.35x | 1.49x | 9.88x | 14.86x |
| heavy | hugo-docs | 995 | 1.7 | 68.5 | 5.63x | 1.40x | 8.98x | 13.22x |
| heavy | geomean | | | | 5.39x | 1.48x | 9.67x | 14.16x |

All numbers were measured on 2026-10-19 on an Intel Core i7-14700KF running Linux and Node 26.8.2. They come from `bench/run.mjs` in the mdust repository, run as `RUNS=7 CPU=10 node run.mjs` for the first two sets and with `CORPORA=` set to the ten heavy corpora for the third. Each cell is the median of 7 runs in alternating order, pinned to core 10 with `taskset`, and each run warms up for 1.5 s and then times full passes over the corpus for at least 2 s.

A multiplier is the markdown-it time divided by the other renderer's time. markdown-wasm is version 1.2.0. markdown-it.rs wasm is the `markdown-it` Rust crate 0.6.1 built to wasm. mdust-rs is this package, called through its Node entry. mdust native is the same crate as a native x86-64 binary with mimalloc and profile-guided optimization, and it is not in this package. Its profile was trained on rust-book and node-api. No corpus in the holdout set was used to tune mdust. A second run of the heavy set the same day gave geomeans of 9.73x for mdust-rs and 14.34x for mdust native.

## What it does not do

- It has no options, no plugins and no streaming API. Each call takes a whole string and returns a whole string.
- Its output matches one markdown-it configuration, `default` with `{ html: true }`. With `html: true`, raw HTML in the input reaches the output unchanged, so sanitize the result before you insert untrusted input into a page.
- Panics abort. In wasm the panic handler runs the `unreachable` instruction, so `render` throws a `WebAssembly.RuntimeError` instead of returning. The next call resets the input, output and allocator, but no test in this repository renders after a trap.
- Wasm memory never shrinks. After a large document, the module keeps the memory it grew to.

## License

MIT
