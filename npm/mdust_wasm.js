// Loader for mdust_wasm_bg.wasm. Strings cross the boundary through two
// buffers the wasm side keeps between calls, so a render costs one encode
// and one decode and no allocation on the JS side.
const { readFileSync } = require('node:fs');
const { join } = require('node:path');

const wasm = new WebAssembly.Instance(
  new WebAssembly.Module(readFileSync(join(__dirname, 'mdust_wasm_bg.wasm'))),
  {},
).exports;

let buf = Buffer.from(wasm.memory.buffer);

function render(src) {
  src = String(src);
  // utf8Write never needs more than 3 bytes per UTF-16 unit.
  const cap = src.length * 3;
  const inPtr = wasm.mdust_in(cap);
  if (buf.buffer !== wasm.memory.buffer) buf = Buffer.from(wasm.memory.buffer);
  const len = buf.utf8Write(src, inPtr, cap);
  const outPtr = wasm.mdust_render(len);
  if (buf.buffer !== wasm.memory.buffer) buf = Buffer.from(wasm.memory.buffer);
  return buf.utf8Slice(outPtr, outPtr + wasm.mdust_out_len());
}

module.exports.render = render;
// Node instantiates on load; init exists so code shared with the browser entry runs unchanged.
module.exports.init = async () => {};
module.exports.__wasm = wasm;
