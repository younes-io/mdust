import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const root = new URL('./corpus/', import.meta.url).pathname;

function walk(dir) {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f);
    return statSync(p).isDirectory() ? walk(p) : p.endsWith('.md') ? [p] : [];
  }).sort();
}

export const corpora = {
  'rust-book': () => walk(join(root, 'rust-book/src')),
  'cm-spec': () => [join(root, 'spec.md')],
  'mdit-samples': () => walk(join(root, 'mdit-src/benchmark/samples')),
  'node-api': () => walk(join(root, 'node-api/doc/api')),
  'mdn-js': () => walk(join(root, 'mdn-js/files/en-us/web/javascript/reference')),
  'k8s-concepts': () => walk(join(root, 'k8s-concepts/content/en/docs/concepts')),
  awesome: () => walk(join(root, 'awesome')),
};

export function loadCorpus(name) {
  return corpora[name]().map((path) => ({ path, src: readFileSync(path, 'utf8') }));
}
