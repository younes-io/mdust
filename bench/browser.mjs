// Render rust-book and node-api with the web glue in headless Chromium, check every
// output hash against the Node glue, and time the web glue against markdown-it in the
// same page (alternating, median of RUNS, each sample at least MIN_MS of full passes).
// usage: RUNS=7 MIN_MS=1000 CPU=10 CHROMIUM=/usr/bin/chromium node browser.mjs
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { extname, join } from 'node:path';
import { loadCorpus } from './corpora.mjs';

const RUNS = Number(process.env.RUNS ?? 7);
const MIN_MS = Number(process.env.MIN_MS ?? 1000);
const CPU = process.env.CPU ?? '10';
const CHROMIUM = process.env.CHROMIUM ?? '/usr/bin/chromium';
const CORPORA = ['rust-book', 'node-api'];
const pkg = new URL('../crates/mdust-wasm/pkg/', import.meta.url).pathname;
const mdit = new URL('node_modules/markdown-it/dist/browser/markdown-it.esm.min.mjs', import.meta.url).pathname;

const docs = Object.fromEntries(CORPORA.map((c) => [c, loadCorpus(c).map((d) => d.src)]));
const nodeRender = createRequire(import.meta.url)(join(pkg, 'mdust_wasm.js')).render;
const sha = (s) => createHash('sha256').update(s, 'utf8').digest('hex');
const expected = Object.fromEntries(CORPORA.map((c) => [c, docs[c].map((s) => sha(nodeRender(s)))]));

const page = `<!doctype html><script type="module">
import { init, render } from '/pkg/mdust_wasm.web.mjs';
import MarkdownIt from '/markdown-it.mjs';
const docs = await (await fetch('/docs.json')).json();
await init();
const md = new MarkdownIt('default', { html: true });
const impls = { mdust: render, 'markdown-it': (s) => md.render(s) };
const enc = new TextEncoder();
const hex = (b) => Array.from(new Uint8Array(b), (x) => x.toString(16).padStart(2, '0')).join('');
const out = { hashes: {}, samples: {}, chars: {} };
for (const [c, srcs] of Object.entries(docs)) {
  out.hashes[c] = await Promise.all(srcs.map(async (s) => hex(await crypto.subtle.digest('SHA-256', enc.encode(render(s))))));
  out.samples[c] = { mdust: [], 'markdown-it': [] };
  out.chars[c] = {};
  const pass = (k) => { let n = 0; for (const s of srcs) n += impls[k](s).length; out.chars[c][k] = n; };
  for (const k in impls) for (const end = performance.now() + 1500; performance.now() < end;) pass(k);
  for (let r = 0; r < ${RUNS}; r++) {
    for (const k of r % 2 ? ['markdown-it', 'mdust'] : ['mdust', 'markdown-it']) {
      let iters = 0; const t0 = performance.now(); let t1 = t0;
      while (t1 - t0 < ${MIN_MS}) { pass(k); iters++; t1 = performance.now(); }
      out.samples[c][k].push((t1 - t0) / iters);
    }
  }
}
await fetch('/result', { method: 'POST', body: JSON.stringify({ ua: navigator.userAgent, ...out }) });
</script>`;

const types = { '.mjs': 'text/javascript', '.js': 'text/javascript', '.wasm': 'application/wasm' };
let done;
const result = new Promise((r) => { done = r; });
const server = createServer((req, res) => {
  const send = (body, type) => res.writeHead(200, { 'Content-Type': type }).end(body);
  if (req.url === '/') return send(page, 'text/html');
  if (req.url === '/docs.json') return send(JSON.stringify(docs), 'application/json');
  if (req.url === '/markdown-it.mjs') return send(readFileSync(mdit), 'text/javascript');
  if (req.url.startsWith('/pkg/')) return send(readFileSync(join(pkg, req.url.slice(5))), types[extname(req.url)]);
  if (req.url === '/result') {
    let body = '';
    req.on('data', (d) => { body += d; }).on('end', () => { res.end(); done(JSON.parse(body)); });
    return;
  }
  res.writeHead(404).end();
}).listen(0, '127.0.0.1');
await new Promise((r) => server.once('listening', r));

const profile = mkdtempSync(join(tmpdir(), 'mdust-chromium-'));
const chrome = spawn('taskset', ['-c', CPU, CHROMIUM, '--headless=new', '--no-first-run', '--no-default-browser-check',
  `--user-data-dir=${profile}`, '--disable-background-timer-throttling', '--disable-renderer-backgrounding',
  `http://127.0.0.1:${server.address().port}/`], { stdio: 'ignore' });
const res = await result;
chrome.kill();
server.close();
rmSync(profile, { recursive: true, force: true });

const median = (xs) => [...xs].sort((a, b) => a - b)[xs.length >> 1];
console.log(res.ua);
let exit = 0;
for (const c of CORPORA) {
  const same = res.hashes[c].filter((h, i) => h === expected[c][i]).length;
  if (same !== docs[c].length) exit = 1;
  console.log(`\n${c}: ${same}/${docs[c].length} output hashes match the Node glue, ${RUNS} runs/impl alternating, ${MIN_MS} ms per sample`);
  const base = median(res.samples[c]['markdown-it']);
  for (const k of ['markdown-it', 'mdust']) {
    const s = res.samples[c][k], m = median(s);
    console.log(`  ${k.padEnd(12)} median ${m.toFixed(3).padStart(8)} ms  range ${Math.min(...s).toFixed(3)}-${Math.max(...s).toFixed(3)}  ${(base / m).toFixed(2)}x vs markdown-it  ${res.chars[c][k]} chars out per pass`);
  }
}
process.exit(exit);
