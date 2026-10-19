import { execFileSync } from 'node:child_process';
import { existsSync, statSync } from 'node:fs';
import { corpora, HEAVY, HOLDOUT } from './corpora.mjs';

const RUNS = Number(process.env.RUNS ?? 7);
const MIN_MS = process.env.MIN_MS ?? '2000';
const CPU = process.env.CPU ?? '2';
const selected = process.env.CORPORA ? process.env.CORPORA.split(',') : Object.keys(corpora);
const impls = ['markdown-it', 'markdown-wasm', 'markdown-it-rs-wasm', 'markdown-it-rs-native', 'mdust', 'mdust-native'];
const here = new URL('.', import.meta.url).pathname;
// bench/pgo.sh writes the profile-guided binary; it wins over the plain release build when present.
const mdustNative = [`${here}native/target/release/mdust-native-pgo`, `${here}native/target/release/mdust-native`].find(existsSync);
const natives = { 'markdown-it-rs-native': `${here}native/target/release/mdit-rs-native`, 'mdust-native': mdustNative };

function once(impl, corpus) {
  const cmd = impl in natives
    ? [natives[impl], MIN_MS, ...corpora[corpus]()]
    : ['node', `${here}worker.mjs`, impl, corpus, MIN_MS];
  return JSON.parse(execFileSync('taskset', ['-c', CPU, ...cmd], { encoding: 'utf8' }));
}

const median = (xs) => [...xs].sort((a, b) => a - b)[xs.length >> 1];
const HEADLINE = ['rust-book', 'node-api', 'mdn-js', 'k8s-concepts', 'awesome'];
const speedups = Object.fromEntries(impls.map((i) => [i, {}]));

for (const corpus of selected) {
  const files = corpora[corpus]();
  const bytes = files.reduce((n, f) => n + statSync(f).size, 0);
  const samples = Object.fromEntries(impls.map((i) => [i, []]));
  for (let r = 0; r < RUNS; r++) {
    const order = r % 2 ? [...impls].reverse() : impls;
    for (const impl of order) samples[impl].push(once(impl, corpus).msPerPass);
  }
  const base = median(samples['markdown-it']);
  console.log(`\n${corpus}: ${files.length} docs, ${(bytes / 1024).toFixed(0)} KiB, ${RUNS} runs/impl alternating, pinned to cpu ${CPU}`);
  for (const impl of impls) {
    const s = samples[impl], m = median(s);
    speedups[impl][corpus] = base / m;
    console.log(`  ${impl.padEnd(22)} median ${m.toFixed(3).padStart(8)} ms  range ${Math.min(...s).toFixed(3)}-${Math.max(...s).toFixed(3)}  ${(bytes / 1048576 / (m / 1000)).toFixed(1).padStart(6)} MiB/s  ${(base / m).toFixed(2)}x vs markdown-it`);
  }
}

for (const [label, set] of [['geomean', HEADLINE], ['holdout geomean', HOLDOUT], ['heavy geomean', HEAVY]]) {
  if (!set.every((c) => selected.includes(c))) continue;
  console.log(`\n${label} speedup vs markdown-it across ${set.join(', ')}`);
  for (const impl of impls) {
    const xs = set.map((c) => speedups[impl][c]);
    console.log(`  ${impl.padEnd(22)} ${Math.exp(xs.reduce((a, x) => a + Math.log(x), 0) / xs.length).toFixed(2)}x`);
  }
}
