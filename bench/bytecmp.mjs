// Byte-compare two mdust pkg dirs over every corpus doc.
// usage: node bytecmp.mjs <base pkg dir> <candidate pkg dir> [more candidate dirs...]
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { corpora, loadCorpus } from './corpora.mjs';

const require = createRequire(import.meta.url);
const loadPkg = (dir) => require(resolve(dir, 'mdust_wasm.js')).render;

const [baseDir, ...candidates] = process.argv.slice(2);
if (!baseDir || candidates.length === 0) {
  console.error('usage: node bytecmp.mjs <base pkg dir> <candidate pkg dir>...');
  process.exit(2);
}
const base = loadPkg(baseDir);
let exit = 0;
for (const dir of candidates) {
  const r = loadPkg(dir);
  let docs = 0, same = 0, bytes = 0;
  const diffs = [];
  for (const c of Object.keys(corpora)) {
    for (const { path, src } of loadCorpus(c)) {
      docs++;
      const a = base(src), b = r(src);
      bytes += b.length;
      if (a === b) same++;
      else if (diffs.length < 5) diffs.push(path);
    }
  }
  console.log(`${dir} vs ${baseDir}: ${same}/${docs} docs byte-identical across all corpora, ${bytes} chars out`);
  for (const d of diffs) console.log(`  differs: ${d}`);
  if (same !== docs) exit = 1;
}
process.exit(exit);
