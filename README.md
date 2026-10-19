<p align="center">
<picture>
	<source media="(prefers-color-scheme: dark)" srcset="logo/a-dark.svg">
	<img src="logo/a.svg" width="160" alt="mdust logo">
</picture>
</p>

<h1 align="center">mdust</h1>

<p align="center">The HTML markdown-it writes, byte for byte, about 10 times faster. Rust compiled to WebAssembly.</p>

<p align="center"><a href="https://www.npmjs.com/package/mdust-rs"><img src="https://img.shields.io/npm/v/mdust-rs" alt="npm version of mdust-rs"></a></p>

<p align="center"><em>An experiment. Can Claude Fable 5.1 port markdown-it to Rust, keep every byte, and go faster?</em></p>

<!-- hero:start -->
```text
  markdown-it  |===  1.00x
  mdust-rs     |######################### 10.14x

  headline set, geometric mean, 2026-10-19
```
<!-- hero:end -->

## Swap the call

If you call markdown-it with its `default` preset and `{ html: true }`, replace the call with `render` and every byte of your HTML stays the same.

<table>
<tr><th>markdown-it</th><th>mdust-rs</th></tr>
<tr>
<td valign="top">

```js
import MarkdownIt from 'markdown-it';

const md = new MarkdownIt('default', { html: true });
md.render('# hi'); // '<h1>hi</h1>\n'
```

</td>
<td valign="top">

```js
import { render } from 'mdust-rs';

render('# hi'); // '<h1>hi</h1>\n'
```

</td>
</tr>
</table>

```sh
npm install mdust-rs
```

In Node, the module loads synchronously on import, so `render` works on the first line. CommonJS works too, as `const { render } = require('mdust-rs')`.

In the browser, call `await init()` once before the first `render`. It fetches the wasm file from next to the loader.

```html
<script type="module">
	import { init, render } from './node_modules/mdust-rs/mdust_wasm.web.mjs';

	await init();
	document.body.innerHTML = render('# hi');
</script>
```

Vite and webpack 5 copy the wasm file into your build. The [package README](npm/README.md#use-it-in-the-browser) covers other bundlers and every input `init` accepts.

## Speed

<!-- chart:start -->
```text
Speedup over markdown-it, geometric mean, higher is faster
2026-10-19, Intel Core i7-14700KF, Linux, Node 26.8.2

headline  5 corpora used for tuning
  markdown-it          |===  1.00x
  markdown-wasm        |===============  6.06x
  markdown-it.rs wasm  |====  1.61x
  mdust-rs             |######################### 10.14x
  mdust native         |++++++++++++++++++++++++++++++++++++++ 15.09x

holdout  6 corpora never used for tuning
  markdown-it          |===  1.00x
  markdown-wasm        |===============  5.95x
  markdown-it.rs wasm  |====  1.55x
  mdust-rs             |#########################  9.85x
  mdust native         |+++++++++++++++++++++++++++++++++++ 13.87x

heavy  10 large corpora, 125.8 MiB in all
  markdown-it          |===  1.00x
  markdown-wasm        |=============  5.39x
  markdown-it.rs wasm  |====  1.48x
  mdust-rs             |########################  9.67x
  mdust native         |+++++++++++++++++++++++++++++++++++ 14.16x

# npm package    + build from source
```
<!-- chart:end -->

Each cell is a geometric mean of markdown-it's time divided by the renderer's time. Higher is faster.

| renderer | headline | holdout | heavy |
|---|---:|---:|---:|
| markdown-it 15.0.3 | 1.00x | 1.00x | 1.00x |
| markdown-wasm 1.2.0 | 6.06x | 5.95x | 5.39x |
| markdown-it.rs 0.6.1, wasm | 1.61x | 1.55x | 1.48x |
| **mdust-rs**, npm | **10.14x** | **9.85x** | **9.67x** |
| **mdust native**, from source | **15.09x** | **13.87x** | **14.16x** |

The headline set is the 5 corpora used to judge and tune speed, including the Rust book and the Node API docs. The holdout set is 6 corpora that no tuning step ever used, and mdust-rs keeps 9.85x there, so the speed is not fitted to the headline set. The heavy set is 10 large corpora with 125.8 MiB of Markdown in all, such as MDN, the Kubernetes docs and the freeCodeCamp curriculum.

Native is the same code as a Node add-on. It is not on npm. Build it from [native/](native/README.md).

<details>
<summary>How it was measured</summary>

Measured on 2026-10-19 on an Intel Core i7-14700KF, Linux, Node 26.8.2, with `RUNS=7 CPU=10 node run.mjs` in `bench/`, and with `CORPORA` set to the heavy corpora for that set. Each per-corpus time is the median of 7 runs pinned to one core. Per-corpus results for all 21 corpora are in the [package README](npm/README.md#benchmarks). `node bench/chart.mjs` redraws both charts in this file from `bench/results.json`.

</details>

## Same bytes

```text
40,919 cases       652 spec examples   23 corpora   0 diffs
```

The target is the string that `new MarkdownIt('default', { html: true }).render(src)` returns with markdown-it 15.0.3, and the parity check compared every output with `===` on 2026-10-19. On the 61 spec examples where markdown-it departs from the spec, mdust follows markdown-it.

## The trade

| You get | You give up |
|---|---|
| One wasm file of 179,356 bytes and no runtime dependencies. | No options and no plugins. The output is always the `default` preset with `{ html: true }`. |
| A synchronous `render` in Node, from `import` or `require`. | Raw HTML passes through unchanged. Sanitize untrusted input yourself. |
| The same `render` in the browser after one `await init()`. | A panic throws a `WebAssembly.RuntimeError`. |
| One function, `render(src)`. A string goes in and a string comes out. | No streaming, and wasm memory never shrinks after a large document. |

## Three tricks

**SIMD scanning.** Scanners read 16 bytes per step to find line ends, the end of each run of text, and the bytes HTML escaping must replace.

**Zero copy.** Text that needs no unescaping stays a slice of your input until the renderer writes it out.

**Bump allocator.** Each render resets one allocator to the same base address instead of freeing objects one by one.

The [package README](npm/README.md#what-is-inside) has the design notes.

<details>
<summary>Repository layout and build</summary>

**Layout.**

- `crates/mdust` is the parser and renderer, a `no_std` Rust library.
- `crates/mdust-wasm` exports `mdust::render` to WebAssembly through two buffers that the JS loaders read and write.
- `npm/` is the `mdust-rs` npm package. npm refused the bare name mdust as too close to an existing package. It holds the Node loader `mdust_wasm.js`, its ES module wrapper `mdust_wasm.mjs`, the browser loader `mdust_wasm.web.mjs`, and the types in `index.d.ts`.
- `bench/` holds the parity checks and the benchmark harness, and compares every output against markdown-it.
- `tools/gen_tables.cjs` generates `crates/mdust/src/tables.rs`, the entity and Unicode punctuation tables.
- `crates/mdust-napi` and `native/` build a native Node add-on, marked private in `native/package.json`.
- `logo/` holds three logo concepts as SVG.

**Build.** You need a Rust toolchain with the `wasm32-unknown-unknown` target, and Node with npm.

1. To build the wasm file into `npm/`, run `npm --prefix npm run build` from the repository root.
2. To make the npm tarball, run `npm pack` in `npm/`. It writes `npm/mdust-rs-0.1.1.tgz`.

The build output, `npm/mdust_wasm_bg.wasm` and the tarball, is not committed.

**Check parity and benchmark.** The benchmark harness reads its corpora from `bench/corpus`, which is not committed.

1. In `bench/`, run `npm install` and `./fetch-corpus.sh` to get markdown-it and the corpora.
2. Run `npm run build:mdust`. It builds `npm/` and copies the wasm file and the loaders into `crates/mdust-wasm/pkg`, which the harness loads.
3. To check parity against markdown-it on every fixture suite and corpus, run `node conformance.mjs --impl mdust`.
4. To time every implementation, run `node run.mjs`. `CORPORA=rust-book RUNS=1` limits a run to one corpus and one sample. `CPU=<n>` picks the core that `taskset` pins each run to.
5. To check the browser loader in headless Chromium, run `node browser.mjs`.

`run.mjs` also times markdown-wasm, the markdown-it.rs crate, and a native build of mdust. To build the markdown-it.rs and native binaries, run `npm run build` in `bench/`.

</details>

## License

MIT
