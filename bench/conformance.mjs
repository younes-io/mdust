import { parseArgs } from 'node:util';
import { relative } from 'node:path';
import { load } from './impls.mjs';
import { corpora, loadCorpus } from './corpora.mjs';
import { fixtureSuites } from './fixtures.mjs';

const { values: args } = parseArgs({
  options: {
    impl: { type: 'string', default: 'markdown-it' },
    suite: { type: 'string' },
    json: { type: 'boolean', default: false },
    'show-fails': { type: 'string', default: '0' },
  },
});
const impls = args.impl.split(',');
const showFails = Number(args['show-fails']);
const here = new URL('.', import.meta.url).pathname;

const suites = [
  ...Object.entries(fixtureSuites).map(([name, cases]) => ({ name: `fixture:${name}`, cases })),
  ...Object.keys(corpora).map((name) => ({
    name: `corpus:${name}`,
    cases: () => loadCorpus(name).map(({ path, src }) => ({ id: relative(here, path), src })),
  })),
].filter((s) => !args.suite || args.suite.split(',').some((f) => s.name.includes(f)));

const norm = (h) => h.replace(/\s+/g, ' ').replace(/> </g, '><').trim();

const firstDiff = (a, b) => {
  let i = 0;
  while (i < a.length && i < b.length && a[i] === b[i]) i++;
  return i;
};

const oracle = await load('markdown-it');

function runSuite(render, { name, cases }) {
  const r = { suite: name, total: 0, pass: 0, fail: 0, error: 0, wsInsensitive: 0, oracleVsFixture: null, fails: [] };
  for (const c of cases()) {
    r.total++;
    const expected = oracle(c.src);
    if (c.fixture !== undefined) r.oracleVsFixture = (r.oracleVsFixture ?? 0) + (expected !== c.fixture);
    let actual;
    try {
      actual = render(c.src);
    } catch (e) {
      r.error++;
      r.fails.push({ id: c.id, src: c.src, expected, error: String(e?.message ?? e) });
      continue;
    }
    if (actual === expected) { r.pass++; r.wsInsensitive++; continue; }
    r.fail++;
    if (norm(actual) === norm(expected)) r.wsInsensitive++;
    r.fails.push({ id: c.id, src: c.src, expected, actual, offset: firstDiff(expected, actual) });
  }
  return r;
}

const MAX_SHOWN = 600;
const window = (s, at) => {
  if (s.length <= MAX_SHOWN) return s;
  const from = Math.max(0, at - MAX_SHOWN / 2);
  return `${from ? `[...${from} chars...]` : ''}${s.slice(from, from + MAX_SHOWN)}[...]`;
};

function printFail(f) {
  const at = f.offset ?? 0;
  console.log(`\n  --- ${f.id}${f.error ? ' (error)' : ` (first diff at offset ${f.offset})`}`);
  console.log(`  input:\n${JSON.stringify(window(f.src, 0))}`);
  console.log(`  expected:\n${JSON.stringify(window(f.expected, at))}`);
  console.log(f.error ? `  error: ${f.error}` : `  actual:\n${JSON.stringify(window(f.actual, at))}`);
}

const t0 = performance.now();
const report = [];
let anyFail = false;
for (const impl of impls) {
  let render;
  try {
    render = await load(impl);
  } catch (e) {
    console.error(`${impl}: ${e.message}`);
    process.exitCode = 2;
    continue;
  }
  for (const s of suites) {
    const r = runSuite(render, s);
    anyFail ||= r.fail + r.error > 0;
    report.push({ impl, ...r, fails: r.fails.slice(0, showFails) });
    if (args.json) continue;
    const pct = ((100 * r.pass) / r.total).toFixed(1);
    const ovf = r.oracleVsFixture === null ? '' : `  oracle!=fixture=${r.oracleVsFixture}`;
    console.log(`${impl.padEnd(20)} ${r.suite.padEnd(26)} ${`${r.pass}/${r.total}`.padStart(9)} ${pct.padStart(5)}%  fail=${r.fail} error=${r.error} ws-insensitive=${r.wsInsensitive}${ovf}`);
    r.fails.slice(0, showFails).forEach(printFail);
  }
}
if (args.json) console.log(JSON.stringify(report, null, 2));
else console.log(`\n${((performance.now() - t0) / 1000).toFixed(1)} s`);
if (anyFail && !process.exitCode) process.exitCode = 1;
