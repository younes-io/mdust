import { parseArgs } from 'node:util';
import { createHash } from 'node:crypto';
import { mkdirSync, writeFileSync } from 'node:fs';
import { load } from './impls.mjs';
import { fixtureSuites } from './fixtures.mjs';
import { loadCorpus } from './corpora.mjs';

const { values: args } = parseArgs({
  options: {
    impl: { type: 'string', default: 'markdown-it' },
    seed: { type: 'string', default: '1' },
    iters: { type: 'string' },
    time: { type: 'string' },
    'max-repros': { type: 'string', default: '20' },
  },
});
const seed = Number(args.seed);
const maxIters = args.iters ? Number(args.iters) : args.time ? Infinity : 5000;
const deadline = args.time ? performance.now() + Number(args.time) * 1000 : Infinity;
const maxRepros = Number(args['max-repros']);
const outDir = new URL(`./fuzz-out/${args.impl}/`, import.meta.url).pathname;

function mulberry32(a) {
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const fixtureInputs = Object.values(fixtureSuites).flatMap((cases) => cases().map((c) => c.src));
const fixtureLines = fixtureInputs.flatMap((s) => s.split('\n'));
const corpusLines = ['node-api', 'mdn-js', 'k8s-concepts', 'awesome']
  .flatMap((name) => loadCorpus(name).flatMap((d) => d.src.split('\n')));

const INLINE = [
  '*', '**', '***', '_', '__', '~~', '~', '[', ']', '(', ')', '![', '](', '[x]', '[^1]', '[foo]: /url',
  '`', '``', '```', '<', '>', '<span>', '</span>', '<a href="x">', '</a>', '<!-- c -->', '<br/>', '<?php ?>',
  '&amp;', '&copy;', '&#35;', '&#x22;', '&bogus;', '&', '\\', '\\*', '\\[', '"', "'", '|', '\\|',
  'http://example.com', '<http://a.b>', '<x@y.z>', 'foo', 'bar', 'baz', ' ', '  ', '\t',
  'é', '中文', '😀', ' ', '​', '\u0000', 'ﬀ', 'ẞ', 'İ',
];
const BLOCK_PREFIX = [
  '', '', '', '# ', '###### ', '> ', '>> ', '- ', '* ', '+ ', '1. ', '2) ', '    ', '\t', '  ', ' - ', '   > ',
];
const LINES = ['---', '***', '___', '===', '```', '~~~', '```js', '<div>', '</div>', '<pre>', '</pre>', '', ''];

function makeGen(rand) {
  const pick = (xs) => xs[Math.floor(rand() * xs.length)];
  const int = (lo, hi) => lo + Math.floor(rand() * (hi - lo + 1));
  const inline = () => Array.from({ length: int(1, 8) }, () => pick(INLINE)).join(rand() < 0.5 ? '' : ' ');
  const tableRow = (n) => '|' + Array.from({ length: n }, inline).join('|') + (rand() < 0.7 ? '|' : '');
  const table = () => {
    const n = int(1, 4);
    const delim = '|' + Array.from({ length: n }, () => pick([':-', '-:', ':-:', '---', '-'])).join('|') + '|';
    return [tableRow(n), delim, ...Array.from({ length: int(0, 3) }, () => tableRow(int(1, 5)))].join('\n');
  };
  const block = () => {
    const r = rand();
    if (r < 0.25) return pick(fixtureInputs).replace(/\n$/, '');
    if (r < 0.4) {
      const i = int(0, fixtureLines.length - 1);
      return fixtureLines.slice(i, i + int(1, 4)).join('\n');
    }
    if (r < 0.5) {
      const i = int(0, corpusLines.length - 1);
      return corpusLines.slice(i, i + int(1, 8)).join('\n');
    }
    if (r < 0.6) return table();
    if (r < 0.7) return pick(LINES);
    return Array.from({ length: int(1, 3) }, () => pick(BLOCK_PREFIX) + inline()).join('\n');
  };
  return () => {
    let doc = Array.from({ length: int(1, 6) }, block).map((b) => b + pick(['\n', '\n', '\n\n'])).join('');
    const eol = rand();
    if (eol < 0.05) doc = doc.replace(/\n/g, '\r\n');
    else if (eol < 0.08) doc = doc.replace(/\n/g, () => pick(['\n', '\r\n', '\r']));
    return doc;
  };
}

const oracle = await load('markdown-it');
const render = await load(args.impl);

function outcome(src) {
  const expected = oracle(src);
  try {
    const actual = render(src);
    return { kind: actual === expected ? 'ok' : 'mismatch', expected, actual };
  } catch (e) {
    return { kind: 'error', expected, actual: String(e?.message ?? e) };
  }
}

// Greedy delta debugging: drop chunks of units while the failure kind persists, halving chunk size.
function ddmin(units, fails) {
  let chunk = Math.max(1, units.length >> 1);
  while (true) {
    let removed = false;
    for (let i = 0; i < units.length; ) {
      const candidate = [...units.slice(0, i), ...units.slice(i + chunk)];
      if (candidate.length && fails(candidate.join(''))) { units = candidate; removed = true; }
      else i += chunk;
    }
    if (chunk === 1 && !removed) return units.join('');
    if (!removed) chunk = Math.max(1, chunk >> 1);
  }
}

function minimize(src, kind) {
  const fails = (s) => outcome(s).kind === kind;
  const byLines = ddmin(src.match(/[^\n]*\n|[^\n]+$/g) ?? [src], fails);
  return ddmin([...byLines], fails);
}

const gen = makeGen(mulberry32(seed));
const repros = new Map();
let iters = 0, divergences = 0, errors = 0;
const t0 = performance.now();
while (iters < maxIters && performance.now() < deadline) {
  const src = gen();
  iters++;
  const o = outcome(src);
  if (o.kind === 'ok') continue;
  divergences++;
  if (o.kind === 'error') errors++;
  if (repros.size >= maxRepros) continue;
  const min = minimize(src, o.kind);
  const hash = createHash('sha1').update(min).digest('hex').slice(0, 12);
  if (repros.has(hash)) continue;
  const m = outcome(min);
  repros.set(hash, { iter: iters, kind: o.kind, input: min, expected: m.expected, actual: m.actual });
  mkdirSync(outDir, { recursive: true });
  writeFileSync(`${outDir}${hash}.md`, min);
  writeFileSync(`${outDir}${hash}.json`, JSON.stringify({ impl: args.impl, seed, iter: iters, kind: o.kind, original: src, ...repros.get(hash) }, null, 2));
  console.log(`[iter ${iters}] ${o.kind} ${src.length} -> ${min.length} chars  fuzz-out/${args.impl}/${hash}.md`);
  console.log(`  input:    ${JSON.stringify(min)}\n  expected: ${JSON.stringify(m.expected)}\n  actual:   ${JSON.stringify(m.actual)}`);
}
const secs = ((performance.now() - t0) / 1000).toFixed(1);
console.log(`\n${args.impl} seed=${seed}: ${iters} iterations in ${secs} s, ${divergences} diverging inputs (${errors} errors), ${repros.size} unique minimized repros`);
if (divergences) process.exitCode = 1;
