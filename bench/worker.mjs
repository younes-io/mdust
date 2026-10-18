import { load } from './impls.mjs';
import { loadCorpus } from './corpora.mjs';

const [impl, corpusName, minMs] = process.argv.slice(2);
const render = await load(impl);
const docs = loadCorpus(corpusName).map((d) => d.src);

let sink = 0;
const pass = () => { for (const s of docs) sink += render(s).length; };

const warmEnd = performance.now() + 1500;
while (performance.now() < warmEnd) pass();

let iters = 0;
const t0 = performance.now();
let t1 = t0;
while (t1 - t0 < Number(minMs)) { pass(); iters++; t1 = performance.now(); }

process.stdout.write(JSON.stringify({ msPerPass: (t1 - t0) / iters, iters, sink }));
