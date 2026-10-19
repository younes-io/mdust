// Byte-compare the web glue against the Node glue of one pkg over every corpus doc.
// usage: node bytecmp-web.mjs [pkg dir, default ../crates/mdust-wasm/pkg]
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { corpora, loadCorpus } from './corpora.mjs';

const dir = resolve(process.argv[2] ?? new URL('../crates/mdust-wasm/pkg', import.meta.url).pathname);
const node = createRequire(import.meta.url)(resolve(dir, 'mdust_wasm.js')).render;
const web = await import(pathToFileURL(resolve(dir, 'mdust_wasm.web.mjs')));
await web.init(readFileSync(resolve(dir, 'mdust_wasm_bg.wasm')));

let docs = 0, same = 0, bytes = 0;
const diffs = [];
for (const c of Object.keys(corpora)) {
  for (const { path, src } of loadCorpus(c)) {
    docs++;
    const a = node(src), b = web.render(src);
    bytes += b.length;
    if (a === b) same++;
    else if (diffs.length < 5) diffs.push(path);
  }
}
console.log(`${dir} web glue vs Node glue: ${same}/${docs} docs byte-identical across all corpora, ${bytes} chars out`);
for (const d of diffs) console.log(`  differs: ${d}`);
process.exit(same === docs ? 0 : 1);
