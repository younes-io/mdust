// Writes docs/bench-light.svg and docs/bench-dark.svg from bench/results.json.
import { readFileSync, writeFileSync } from 'node:fs';

const results = JSON.parse(readFileSync(new URL('results.json', import.meta.url), 'utf8'));
const FONT = 'ui-monospace, SFMono-Regular, Menlo, monospace';
const THEMES = {
	light: { text: '#1f2328', muted: '#59636e', grid: '#d1d9e0', other: '#8c959f', rs: '#0969da', native: '#8250df' },
	dark: { text: '#f0f6fc', muted: '#9198a1', grid: '#3d444d', other: '#656c76', rs: '#4493f8', native: '#ab7df8' },
};
const SET_NOTES = {
	headline: 'corpora used for tuning',
	holdout: 'corpora never used for tuning',
	heavy: 'large corpora, 125.8 MiB in all',
};

const W = 700;
const LABEL_RIGHT = 172;
const X0 = 184;
const AXIS_MAX = 15.5;
const SCALE = (W - X0 - 60) / AXIS_MAX;
const BAR = 20;
const STEP = 26;
const GROUP_HEAD = 30;
const GROUP_GAP = 18;
const TOP = 76;

const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;');
const text = (x, y, s, attrs = '') => `<text x="${x}" y="${y}" ${attrs}>${esc(s)}</text>`;

function rows(set) {
	const names = [results.baseline, ...results.renderers];
	const values = [1, ...results.sets[set].geomean];
	const kinds = ['other', 'other', 'other', 'rs', 'native'];
	return names.map((name, i) => ({ name, value: values[i], kind: kinds[i] }));
}

function chart(theme) {
	const c = THEMES[theme];
	const sets = Object.keys(results.sets);
	const groupHeight = GROUP_HEAD + (results.renderers.length + 1) * STEP;
	const plotBottom = TOP + sets.length * groupHeight + (sets.length - 1) * GROUP_GAP;
	const H = plotBottom + 44;
	const out = [];

	sets.forEach((set, g) => {
		const y0 = TOP + g * (groupHeight + GROUP_GAP);
		const count = Object.keys(results.sets[set].corpora).length;
		const barsTop = y0 + GROUP_HEAD - 4;
		out.push(`<line x1="${X0}" y1="${barsTop}" x2="${X0}" y2="${y0 + groupHeight}" stroke="${c.grid}" stroke-width="1"/>`);
		out.push(`<text x="16" y="${y0 + 16}"><tspan fill="${c.text}" font-size="14" font-weight="700">${set}</tspan><tspan dx="10" fill="${c.muted}" font-size="12">${count} ${SET_NOTES[set]}</tspan></text>`);
		rows(set).forEach((row, i) => {
			const y = y0 + GROUP_HEAD + i * STEP;
			const width = row.value * SCALE;
			const strong = row.kind !== 'other';
			out.push(text(LABEL_RIGHT, y + 14, row.name, `fill="${strong ? c.text : c.muted}" font-size="13" text-anchor="end"${strong ? ' font-weight="700"' : ''}`));
			out.push(`<rect x="${X0}" y="${y}" width="${width.toFixed(1)}" height="${BAR}" rx="3" fill="${c[row.kind]}"/>`);
			out.push(text((X0 + width + 6).toFixed(1), y + 14, `${row.value.toFixed(2)}x`, `fill="${strong ? c.text : c.muted}" font-size="13"${strong ? ' font-weight="700"' : ''}`));
		});
	});

	const title = 'Speedup over markdown-it, geometric mean, higher is faster';
	const sub = `${results.date}, ${results.machine}, Node ${results.node}`;
	const foot = 'mdust native is an x86-64 build with profile-guided optimization, not in the npm package.';
	return [
		`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" font-family="${FONT}" role="img" aria-label="${esc(title)}">`,
		`<title>${esc(title)}</title>`,
		text(16, 28, title, `fill="${c.text}" font-size="15" font-weight="700"`),
		text(16, 50, sub, `fill="${c.muted}" font-size="12"`),
		...out,
		text(16, H - 14, foot, `fill="${c.muted}" font-size="12"`),
		'</svg>',
		'',
	].join('\n');
}

for (const theme of Object.keys(THEMES)) {
	writeFileSync(new URL(`../docs/bench-${theme}.svg`, import.meta.url), chart(theme));
}
