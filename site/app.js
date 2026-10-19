import { init, render } from './mdust_wasm.web.mjs';

const sample = `# a small document

mdust renders *emphasis*, **strong text**, \`inline code\` and [links](https://github.com/younes-io/mdust).

- one wasm file
- one function
- the same bytes as markdown-it

\`\`\`js
import { init, render } from 'mdust-rs';

await init();
render('# hi');
\`\`\`

| corpus | markdown-it | mdust-rs |
|---|---:|---:|
| rust-book | 1.00x | 8.40x |
| node-api | 1.00x | 8.84x |
`;

const bigTarget = 200 * 1024;
const encoder = new TextEncoder();
const src = document.getElementById('src');
const out = document.getElementById('out');
const raw = document.getElementById('raw');
const status = document.getElementById('status');
const big = document.getElementById('big');

let html = '';

function show() {
  out.classList.toggle('raw', raw.checked);
  if (raw.checked) out.textContent = html;
  else out.innerHTML = html;
}

function update() {
  const text = src.value;
  const start = performance.now();
  html = render(text);
  const ms = performance.now() - start;
  show();
  const inBytes = encoder.encode(text).length;
  const outBytes = encoder.encode(html).length;
  const time = document.createElement('span');
  time.className = 'ms';
  time.textContent = `${ms.toFixed(2)} ms`;
  status.replaceChildren(
    time,
    ` render. input ${inBytes.toLocaleString('en')} bytes, output ${outBytes.toLocaleString('en')} bytes. this is your browser's own wasm time for the last render, not the benchmark.`,
  );
}

function bigDocument() {
  const copies = Math.ceil(bigTarget / encoder.encode(sample).length);
  return Array(copies).fill(sample).join('\n');
}

src.value = sample;

try {
  await init();
} catch (err) {
  status.textContent = 'could not load the wasm module. serve this page over http, not from a file.';
  throw err;
}

src.addEventListener('input', update);
raw.addEventListener('change', show);
big.addEventListener('click', () => {
  src.value = bigDocument();
  update();
});
update();
