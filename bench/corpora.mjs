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
  // Heavy set: whole trees of ten large, popular corpora. github-docs-full is the github-docs clone unsliced.
  'github-docs-full': () => walk(join(root, 'github-docs/content')),
  'mdn-web': () => walk(join(root, 'mdn-web/files/en-us/web')),
  'k8s-docs': () => walk(join(root, 'k8s-docs/content/en/docs')),
  'vscode-docs': () => walk(join(root, 'vscode-docs/docs')),
  'docker-docs': () => walk(join(root, 'docker-docs/content')),
  // fcc-curriculum is 53 MiB whole; the first 9000 sorted paths are 25 MiB.
  'fcc-curriculum': () => walk(join(root, 'fcc-curriculum/curriculum/challenges/english')).slice(0, 9000),
  eips: () => walk(join(root, 'eips/EIPS')),
  'react-dev': () => walk(join(root, 'react-dev/src/content')),
  ydkjs: () => walk(join(root, 'ydkjs')),
  'hugo-docs': () => walk(join(root, 'hugo-docs/content/en')),
};

export const HOLDOUT = ['github-docs', 'vue-docs', 'rust-rfcs', 'tldr', 'electron-docs', 'obsidian-help'];
export const HEAVY = ['github-docs-full', 'mdn-web', 'k8s-docs', 'vscode-docs', 'docker-docs', 'fcc-curriculum', 'eips', 'react-dev', 'ydkjs', 'hugo-docs'];

export function loadCorpus(name) {
  return corpora[name]().map((path) => ({ path, src: readFileSync(path, 'utf8') }));
}
