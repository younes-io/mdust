import { readFileSync } from 'node:fs';
import MarkdownIt from 'markdown-it';

const dir = new URL('./corpus/mdit-src/test/fixtures/', import.meta.url).pathname;

// Port of markdown-it test/helpers.mjs parseFixtureFile: header, '.', input, '.', expected, '.'.
function parseDotFixtures(text, file) {
  const lines = text.split(/\r?\n/g);
  const withLF = (ls) => (ls.length ? ls.join('\n') + '\n' : '');
  const cases = [];
  let i = 0;
  while (i < lines.length) {
    if (lines[i] !== '.') { i++; continue; }
    const start = ++i;
    while (i < lines.length && lines[i] !== '.') i++;
    if (i >= lines.length) break;
    const src = withLF(lines.slice(start, i));
    const expStart = ++i;
    while (i < lines.length && lines[i] !== '.') i++;
    if (i >= lines.length) break;
    const fixture = withLF(lines.slice(expStart, i));
    i++;
    cases.push({ id: `${file}:${start}`, src, fixture });
  }
  return cases;
}

// spec.txt is the CommonMark spec source, not dot triples. Port of test/cmspec/commonmark.test.mjs.
function parseSpecExamples(text, file) {
  const md = new MarkdownIt('commonmark');
  return md.parse(text.replace(/→/g, '\t'), {})
    .filter((t) => t.tag === 'code' && t.info.trim() === 'example')
    .map((t) => {
      const [src, html] = t.content.split(/^\.\s*?$/m, 2);
      return { id: `${file}:${t.map[0] + 1}`, src, fixture: html.replace(/^\n/, '') };
    });
}

const read = (f) => readFileSync(dir + f, 'utf8');

export const fixtureSuites = {
  'cm-spec': () => parseSpecExamples(read('commonmark/spec.txt'), 'commonmark/spec.txt'),
  'mdit-tables': () => parseDotFixtures(read('markdown-it/tables.txt'), 'markdown-it/tables.txt'),
  'mdit-strikethrough': () => parseDotFixtures(read('markdown-it/strikethrough.txt'), 'markdown-it/strikethrough.txt'),
  'mdit-cm-extras': () => parseDotFixtures(read('markdown-it/commonmark_extras.txt'), 'markdown-it/commonmark_extras.txt'),
};
