import { createRequire } from 'node:module';
import MarkdownIt from 'markdown-it';
import * as mdwasm from 'markdown-wasm';

const require = createRequire(import.meta.url);

export async function load(name) {
  switch (name) {
    case 'markdown-it': {
      const md = new MarkdownIt('default', { html: true });
      return (s) => md.render(s);
    }
    case 'markdown-wasm': {
      await mdwasm.ready;
      const parseFlags = mdwasm.ParseFlags.STRIKETHROUGH | mdwasm.ParseFlags.TABLES;
      return (s) => mdwasm.parse(s, { parseFlags });
    }
    case 'markdown-it-rs-wasm':
      return require('./rust-wasm/pkg/mdit_rs_wasm.js').render;
    default:
      throw new Error(`unknown impl ${name}`);
  }
}
