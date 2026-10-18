import { createRequire } from 'node:module';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import MarkdownIt from 'markdown-it';
import * as mdwasm from 'markdown-wasm';

const require = createRequire(import.meta.url);

export const MDUST_PKG = process.env.MDUST_PKG
  ? resolve(process.env.MDUST_PKG, 'mdust_wasm.js')
  : new URL('../crates/mdust-wasm/pkg/mdust_wasm.js', import.meta.url).pathname;

const oracle = () => {
  const md = new MarkdownIt('default', { html: true });
  return (s) => md.render(s);
};

const hash32 = (s) => {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619);
  return h >>> 0;
};

function controlBroken() {
  const render = oracle();
  return (s) => {
    let out = render(s)
      .replaceAll('style="text-align:right"', 'style="text-align:left"')
      .replaceAll('&amp;', '&#38;');
    if (hash32(s) % 7 === 0) out = out.replace('</p>', '');
    return out;
  };
}

function mdust() {
  if (!existsSync(MDUST_PKG)) {
    throw new Error(`mdust not built: ${MDUST_PKG} is missing. Build crates/mdust-wasm with wasm-bindgen --target nodejs first.`);
  }
  return require(MDUST_PKG).render;
}

export async function load(name) {
  switch (name) {
    case 'markdown-it':
      return oracle();
    case 'control-broken':
      return controlBroken();
    case 'markdown-wasm': {
      await mdwasm.ready;
      const parseFlags = mdwasm.ParseFlags.STRIKETHROUGH | mdwasm.ParseFlags.TABLES;
      return (s) => mdwasm.parse(s, { parseFlags });
    }
    case 'markdown-it-rs-wasm':
      return require('./rust-wasm/pkg/mdit_rs_wasm.js').render;
    case 'mdust':
      return mdust();
    default:
      throw new Error(`unknown impl ${name}`);
  }
}
