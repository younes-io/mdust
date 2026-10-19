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
  // Holdouts: measured, never tuned against. github-docs is 20 MiB; the first 1200 sorted paths are 6.6 MiB.
  'github-docs': () => walk(join(root, 'github-docs/content')).slice(0, 1200),
  'vue-docs': () => walk(join(root, 'vue-docs/src')),
  'rust-rfcs': () => walk(join(root, 'rust-rfcs/text')),
  tldr: () => [...walk(join(root, 'tldr/pages/common')), ...walk(join(root, 'tldr/pages/linux'))],
  'electron-docs': () => walk(join(root, 'electron-docs/docs')),
  'obsidian-help': () => walk(join(root, 'obsidian-help/en')),
};

export const HOLDOUT = ['github-docs', 'vue-docs', 'rust-rfcs', 'tldr', 'electron-docs', 'obsidian-help'];

export function loadCorpus(name) {
  return corpora[name]().map((path) => ({ path, src: readFileSync(path, 'utf8') }));
}
