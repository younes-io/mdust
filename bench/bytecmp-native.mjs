// Byte-compare a native mdust binary's output against the WASM pkg over every corpus doc.
// usage: node bytecmp-native.mjs <mdust-native binary> [pkg dir]   (pkg dir defaults to MDUST_PKG)
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { corpora, loadCorpus } from './corpora.mjs';
import { load } from './impls.mjs';

const [bin, pkgDir] = process.argv.slice(2);
if (!bin) {
  console.error('usage: node bytecmp-native.mjs <mdust-native binary> [pkg dir]');
  process.exit(2);
}
if (pkgDir) process.env.MDUST_PKG = pkgDir;
const wasm = await load('mdust');
const dump = mkdtempSync(join(process.env.CLAUDE_JOB_DIR ? join(process.env.CLAUDE_JOB_DIR, 'tmp') : tmpdir(), 'mdust-dump-'));
let docs = 0, same = 0, bytes = 0;
const diffs = [];
for (const c of Object.keys(corpora)) {
  execFileSync(bin, ['0', ...corpora[c]()], { env: { ...process.env, DUMP: dump } });
  for (const { path, src } of loadCorpus(c)) {
    docs++;
    const a = wasm(src);
    const b = readFileSync(join(dump, path.replace(/^\//, '') + '.html'), 'utf8');
    bytes += b.length;
    if (a === b) same++;
    else if (diffs.length < 5) diffs.push(path);
  }
}
rmSync(dump, { recursive: true });
console.log(`${bin} vs wasm pkg: ${same}/${docs} docs byte-identical across all corpora, ${bytes} chars out`);
for (const d of diffs) console.log(`  differs: ${d}`);
process.exit(same === docs ? 0 : 1);
