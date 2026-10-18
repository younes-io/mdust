// A/B two mdust builds with the same pinning as run.mjs: alternating order, median of
// RUNS, per corpus, plus the geomean of B/A over the corpora. A side is a WASM pkg dir
// or a native binary with the mdust-native contract.
// usage: RUNS=7 CPU=10 CORPORA=rust-book,awesome node ab.mjs <pkg dir or binary A> <B>
import { execFileSync } from 'node:child_process';
import { statSync } from 'node:fs';
import { resolve } from 'node:path';
import { corpora } from './corpora.mjs';

const RUNS = Number(process.env.RUNS ?? 7);
const MIN_MS = process.env.MIN_MS ?? '2000';
const CPU = process.env.CPU ?? '2';
const CORPORA = (process.env.CORPORA ?? 'rust-book,node-api,mdn-js,k8s-concepts,awesome').split(',');
const here = new URL('.', import.meta.url).pathname;
const pkgs = process.argv.slice(2).map((p) => resolve(p));
if (pkgs.length !== 2) {
  console.error('usage: node ab.mjs <pkg A> <pkg B>');
  process.exit(2);
}

function once(side, corpus) {
  const cmd = statSync(side).isFile()
    ? [side, MIN_MS, ...corpora[corpus]()]
    : ['node', `${here}worker.mjs`, 'mdust', corpus, MIN_MS];
  const out = execFileSync('taskset', ['-c', CPU, ...cmd], {
    encoding: 'utf8', env: { ...process.env, MDUST_PKG: side },
  });
  return JSON.parse(out);
}
const median = (xs) => [...xs].sort((a, b) => a - b)[xs.length >> 1];

const ratios = [];
for (const corpus of CORPORA) {
  const samples = [[], []];
  let sink = 0;
  for (let r = 0; r < RUNS; r++) {
    for (const k of r % 2 ? [1, 0] : [0, 1]) {
      const res = once(pkgs[k], corpus);
      samples[k].push(res.msPerPass);
      sink = res.sink;
    }
  }
  const [a, b] = samples.map(median);
  ratios.push(b / a);
  const fmt = (s) => `${median(s).toFixed(3).padStart(8)} ms range ${Math.min(...s).toFixed(3)}-${Math.max(...s).toFixed(3)}`;
  console.log(`${corpus.padEnd(13)} A ${fmt(samples[0])}  B ${fmt(samples[1])}  B/A ${(b / a).toFixed(4)}  work ${sink} chars`);
}
console.log(`geomean B/A over ${CORPORA.join(',')}: ${Math.exp(ratios.reduce((s, x) => s + Math.log(x), 0) / ratios.length).toFixed(4)}`);
