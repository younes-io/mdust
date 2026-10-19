// Draws the ASCII benchmark chart from bench/results.json into the root README,
// between the chart:start and chart:end markers.
import { readFileSync, writeFileSync } from 'node:fs';

const results = JSON.parse(readFileSync(new URL('results.json', import.meta.url), 'utf8'));
const README = new URL('../README.md', import.meta.url);
const SET_NOTES = {
	headline: 'corpora used for tuning',
	holdout: 'corpora never used for tuning',
	heavy: 'large corpora, 125.8 MiB in all',
};
const GLYPH = ['-', '-', '-', '#', '+'];
const COLS_PER_X = 2.5;

const names = [results.baseline, ...results.renderers];
const label = Math.max(...names.map((n) => n.length));
const lines = [
	'Speedup over markdown-it, geometric mean, higher is faster',
	`${results.date}, ${results.machine}, Node ${results.node}`,
];

for (const [set, data] of Object.entries(results.sets)) {
	const count = Object.keys(data.corpora).length;
	lines.push('', `${set}  ${count} ${SET_NOTES[set]}`);
	[1, ...data.geomean].forEach((value, i) => {
		const bar = GLYPH[i].repeat(Math.round(value * COLS_PER_X));
		lines.push(`  ${names[i].padEnd(label)}  |${bar} ${value.toFixed(2).padStart(5)}x`);
	});
}
lines.push('', '# mdust-rs, this package    + mdust native, not in the npm package');

const block = ['<!-- chart:start -->', '```text', ...lines, '```', '<!-- chart:end -->'].join('\n');
const readme = readFileSync(README, 'utf8');
const next = readme.replace(/<!-- chart:start -->[\s\S]*?<!-- chart:end -->/, block);
if (next === readme) throw new Error('chart markers not found in README.md');
writeFileSync(README, next);
