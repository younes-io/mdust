// Browser loader for mdust_wasm_bg.wasm, the same two-buffer protocol as
// mdust_wasm.js with TextEncoder and TextDecoder in place of Buffer.
const encoder = new TextEncoder();
// Buffer.utf8Slice keeps a leading U+FEFF; so must this.
const decoder = new TextDecoder('utf-8', { ignoreBOM: true });

let wasm;
let bytes;

export async function init(source = new URL('mdust_wasm_bg.wasm', import.meta.url)) {
  if (typeof source === 'string' || source instanceof URL) source = fetch(source);
  source = await source;
  let module = source;
  if (typeof Response === 'function' && source instanceof Response) {
    // compileStreaming rejects any other content type, as served by many static hosts.
    module = source.headers.get('Content-Type')?.startsWith('application/wasm')
      ? await WebAssembly.compileStreaming(source)
      : await WebAssembly.compile(await source.arrayBuffer());
  } else if (!(source instanceof WebAssembly.Module)) {
    module = await WebAssembly.compile(source);
  }
  wasm = (await WebAssembly.instantiate(module, {})).exports;
  bytes = new Uint8Array(wasm.memory.buffer);
}

export function render(src) {
  src = String(src);
  // encodeInto never needs more than 3 bytes per UTF-16 unit.
  const cap = src.length * 3;
  const inPtr = wasm.mdust_in(cap);
  if (bytes.buffer !== wasm.memory.buffer) bytes = new Uint8Array(wasm.memory.buffer);
  const { written } = encoder.encodeInto(src, bytes.subarray(inPtr, inPtr + cap));
  const outPtr = wasm.mdust_render(written);
  if (bytes.buffer !== wasm.memory.buffer) bytes = new Uint8Array(wasm.memory.buffer);
  return decoder.decode(bytes.subarray(outPtr, outPtr + wasm.mdust_out_len()));
}
