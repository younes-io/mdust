/** Renders CommonMark to HTML, byte-identical to markdown-it 15.0.3 with preset `default` and `{ html: true }`. */
export function render(src: string): string;

/**
 * Loads the wasm module. The browser entry needs `await init()` before the first `render`.
 * The Node entries load the module on import, so there `init` resolves at once.
 * In the browser, `source` defaults to `mdust_wasm_bg.wasm` next to the entry file.
 */
export function init(
  source?: string | URL | Response | Promise<Response> | WebAssembly.Module | BufferSource,
): Promise<void>;
