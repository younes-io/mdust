// Draws the ASCII benchmark charts from bench/results.json into the root README:
// the two-row hero between the hero markers and the full chart between the chart markers.
import { readFileSync, writeFileSync } from 'node:fs';

const results = JSON.parse(readFileSync(new URL('results.json', import.meta.url), 'utf8'));
const README = new URL('../README.md', import.meta.url);
const SET_NOTES = {
	headline: 'corpora used for tuning',
	holdout: 'corpora never used for tuning',
	heavy: 'large corpora, 125.8 MiB in all',
};
const GLYPH = ['=', '=', '=', '#', '+'];
const COLS_PER_X = 2.5;

const names = [results.baseline, ...results.renderers];

const rows = (indices, values) => {
	const label = Math.max(...indices.map((i) => names[i].length));
	return indices.map((i) => {
		const bar = GLYPH[i].repeat(Math.round(values[i] * COLS_PER_X));
		return `  ${names[i].padEnd(label)}  |${bar} ${values[i].toFixed(2).padStart(5)}x`;
	});
};

const all = names.map((_, i) => i);
const chart = [
	'Speedup over markdown-it, geometric mean, higher is faster',
	`${results.date}, ${results.machine}, Node ${results.node}`,
];
for (const [set, data] of Object.entries(results.sets)) {
	const count = Object.keys(data.corpora).length;
	chart.push('', `${set}  ${count} ${SET_NOTES[set]}`, ...rows(all, [1, ...data.geomean]));
}
chart.push('', '# npm package    + build from source');

const npm = names.indexOf('mdust-rs');
const hero = [
	...rows([0, npm], [1, ...results.sets.headline.geomean]),
	'',
	`  headline set, geometric mean, ${results.date}`,
];

const fill = (text, marker, lines) => {
	const pattern = new RegExp(`<!-- ${marker}:start -->[\\s\\S]*?<!-- ${marker}:end -->`);
	if (!pattern.test(text)) throw new Error(`${marker} markers not found in README.md`);
	const block = [`<!-- ${marker}:start -->`, '```text', ...lines, '```', `<!-- ${marker}:end -->`].join('\n');
	return text.replace(pattern, () => block);
};

const readme = readFileSync(README, 'utf8');
writeFileSync(README, fill(fill(readme, 'hero', hero), 'chart', chart));
