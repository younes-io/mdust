# mdust-native

mdust as a native Node add-on. It exposes the same `render(src: string): string` as the `mdust` wasm package, and its output is byte-identical to it.

This package is not published to npm and ships no prebuilt binaries. You build it from source on your own machine.

## Build

You need a Rust toolchain and Node.js.

```sh
cd native
npm install
npm run build
```

The build compiles `crates/mdust-napi` and writes `mdust-native.node` next to `index.js`.

## Use

```js
const { render } = require('./native');

render('# hi'); // '<h1>hi</h1>\n'
```

## License

MIT
