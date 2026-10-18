import { load } from './impls.mjs';
import { corpora, loadCorpus } from './corpora.mjs';

const norm = (h) => h.replace(/\s+/g, ' ').replace(/> </g, '><').trim();
const ref = await load('markdown-it');

for (const impl of ['markdown-wasm', 'markdown-it-rs-wasm']) {
  const render = await load(impl);
  for (const c of Object.keys(corpora)) {
    const docs = loadCorpus(c);
    let exact = 0, loose = 0, errors = 0;
    for (const { src } of docs) {
      try {
        const a = ref(src), b = render(src);
        if (a === b) exact++;
        if (norm(a) === norm(b)) loose++;
      } catch { errors++; }
    }
    console.log(`${impl.padEnd(20)} ${c.padEnd(13)} docs=${docs.length} exact=${exact} whitespace-insensitive=${loose} errors=${errors}`);
  }
}
