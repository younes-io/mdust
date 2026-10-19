# mdust

mdust renders Markdown to HTML in WebAssembly. Its output is byte-identical to [markdown-it](https://github.com/markdown-it/markdown-it) 15.0.3 with preset `default` and `{ html: true }`. On the corpora in the benchmark section, it is about 10 times faster than markdown-it in Node.

## Install

```sh
npm install mdust
```

The package has no dependencies. It contains one wasm file of about 180 kB and three small loaders. The tarball is about 80 kB.

## Use it in Node

The Node entry loads the wasm module synchronously on import. You can call `render` right away.

CommonJS:

```js
const { render } = require('mdust');

render('# hi'); // '<h1>hi</h1>\n'
```

ES modules:

```js
import { render } from 'mdust';

render('# hi'); // '<h1>hi</h1>\n'
```

## Use it in the browser

The browser entry fetches and compiles the wasm module asynchronously. Call `await init()` once before the first `render`. This is the only API difference between the Node and browser entries. The Node entries also export `init`, which resolves at once, so code shared between the two can always call it.

With no argument, `init` fetches `mdust_wasm_bg.wasm` from the directory of `mdust_wasm.web.mjs`. If you serve the package files as they are, a plain module script is enough:

```html
<script type="module">
	import { init, render } from './node_modules/mdust/mdust_wasm.web.mjs';

	await init();
	document.body.innerHTML = render('# hi');
</script>
```

If the wasm file lives somewhere else, pass its location to `init`:

```js
await init(new URL('/assets/mdust_wasm_bg.wasm', import.meta.url));
```

`init` also accepts a `Response`, a promise of a `Response`, a compiled `WebAssembly.Module`, or the wasm bytes as an `ArrayBuffer` or typed array. If the server sends the file with `Content-Type: application/wasm`, `init` compiles it while it downloads. Otherwise it downloads the whole file first.

Bundlers pick the browser entry through the `browser` and `default` export conditions. A bundler that rewrites `new URL('file', import.meta.url)`, such as Vite or webpack 5, copies the wasm file into the build output by itself. With other bundlers, copy `mdust/mdust_wasm_bg.wasm` into your static assets and pass its URL to `init`. The bundler setups in this paragraph are not tested in this repository.

## Parity with markdown-it

`render(src)` returns the same string, byte for byte, as this markdown-it call:

```js
new MarkdownIt('default', { html: true }).render(src)
```

The reference is markdown-it 15.0.3. The repository checks parity on every document of its benchmark corpora and on the CommonMark spec and markdown-it fixture suites. There is no option to change the output. If you need other markdown-it options or plugins, use markdown-it.

## Benchmarks

All numbers below were measured on 2026-10-19 on one machine: an Intel Core i7-14700KF, Node 26.8.2, Linux, one pinned core. They come from `bench/run.mjs` in the mdust repository. Each number is the median of 7 alternating runs. Each run warms up for 1.5 s, then times full passes over the corpus for at least 2 s. Speedup is the markdown-it time divided by the other time. `mdust` in these tables is the wasm build in this package, called through the Node entry.

Heavy corpora, measured with this command in `bench/`:

```sh
RUNS=7 CPU=10 CORPORA=github-docs-full,mdn-web,k8s-docs,vscode-docs,docker-docs,fcc-curriculum,eips,react-dev,ydkjs,hugo-docs node run.mjs
```

| corpus | docs | MiB | markdown-it ms | markdown-wasm ms | speedup | mdust ms | speedup |
|---|---:|---:|---:|---:|---:|---:|---:|
| github-docs-full | 3760 | 19.6 | 607.5 | 95.3 | 6.37x | 56.9 | 10.68x |
| mdn-web | 11005 | 39.7 | 1202.5 | 212.9 | 5.65x | 119.0 | 10.10x |
| k8s-docs | 1722 | 15.5 | 244.7 | 52.6 | 4.65x | 26.4 | 9.25x |
| vscode-docs | 364 | 4.8 | 148.5 | 24.4 | 6.09x | 14.9 | 9.97x |
| docker-docs | 1095 | 7.9 | 267.1 | 43.6 | 6.12x | 25.7 | 10.40x |
| fcc-curriculum | 9000 | 25.1 | 497.1 | 115.8 | 4.29x | 53.7 | 9.25x |
| eips | 966 | 6.5 | 166.0 | 28.6 | 5.81x | 16.5 | 10.08x |
| react-dev | 225 | 3.8 | 82.2 | 18.6 | 4.41x | 9.855 | 8.34x |
| ydkjs | 55 | 0.88 | 19.5 | 3.638 | 5.35x | 1.971 | 9.88x |
| hugo-docs | 995 | 1.7 | 68.5 | 12.2 | 5.63x | 7.636 | 8.98x |
| geomean | | | | | 5.39x | | 9.67x |

Geometric mean speedup over markdown-it on the other two corpus sets, measured with `RUNS=7 CPU=10 node run.mjs` in `bench/`:

| corpus set | corpora | markdown-wasm | mdust |
|---|---|---:|---:|
| headline | rust-book, node-api, mdn-js, k8s-concepts, awesome | 6.06x | 10.14x |
| holdout | github-docs, vue-docs, rust-rfcs, tldr, electron-docs, obsidian-help | 5.95x | 9.85x |

The holdout corpora were never used to tune mdust. These numbers are for Node. `bench/browser.mjs` in the repository times the browser entry in headless Chromium.

## License

MIT
