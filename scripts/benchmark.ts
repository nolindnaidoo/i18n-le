/**
 * Measure real throughput. Run with `bun run benchmark`.
 *
 * Numbers are machine-specific, so the host is recorded alongside them and
 * they are never asserted in CI — a benchmark that gates a build just fails on
 * a slower runner. The point is a reproducible figure, not a pass/fail.
 *
 * Inputs are generated rather than checked in so the sizes are explicit and
 * the corpus cannot silently drift from what the numbers claim.
 */
import { cpus, totalmem } from 'node:os';
import { type CatalogueDocument, reportFor } from '../src/audit/report';
import type { LibraryId } from '../src/audit/library';

interface Case {
	readonly label: string;
	readonly library: LibraryId;
	readonly build: () => CatalogueDocument[];
}

const flat = (keys: number, value: (i: number) => string) =>
	JSON.stringify(Object.fromEntries(Array.from({ length: keys }, (_, i) => [`section${i % 40}.key${i}`, value(i)])), null, 2);

const CASES: readonly Case[] = [
	{
		label: 'i18next, 25 locales',
		library: 'i18next',
		build: () =>
			['en', ...Array.from({ length: 24 }, (_, i) => `l${String.fromCharCode(97 + i)}`)].map((locale, index) => ({
				name: `${locale}.json`,
				locale,
				content: flat(2_000, (i) => (index === 0 || i % 7 ? `Text ${i} for {{name}} and {{count}}` : `Text ${i} {{nombre}}`)),
			})),
	},
	{
		label: 'next-intl ICU, 2 locales',
		library: 'next-intl',
		build: () =>
			['en', 'es'].map((locale) => ({
				name: `${locale}.json`,
				locale,
				content: flat(20_000, (i) => `{count, plural, one {# item ${i}} other {# items ${i}}} for {name}`),
			})),
	},
	{
		label: 'VS Code bundle, 12 locales',
		library: 'vscode-l10n',
		build: () =>
			[undefined, 'de', 'es', 'fr', 'id', 'it', 'ja', 'ko', 'pt-BR', 'ru', 'uk', 'vi', 'zh-CN'].map((locale) => ({
				name: locale ? `bundle.l10n.${locale.toLowerCase()}.json` : 'bundle.l10n.json',
				locale,
				content: JSON.stringify(
					Object.fromEntries(Array.from({ length: 1_500 }, (_, i) => [`Open {0} files in folder ${i}`, `${locale ?? 'en'} ${locale && i % 9 === 0 ? '' : '{0}'} ${i}`])),
					null,
					2,
				),
			})),
	},
];

const WARMUP = 2;
const RUNS = 7;

function median(xs: readonly number[]): number {
	const s = [...xs].sort((a, b) => a - b);
	const mid = Math.floor(s.length / 2);
	return s.length % 2 ? (s[mid] as number) : ((s[mid - 1] as number) + (s[mid] as number)) / 2;
}

const run = (documents: CatalogueDocument[], library: LibraryId) =>
	reportFor(documents, { library, version: null, layout: null, keysAreSource: library === 'vscode-l10n', evidence: [] }, {
		source: undefined,
		keysAreSource: false,
	}).findings.length;

const results: Array<Record<string, unknown>> = [];
for (const c of CASES) {
	const documents = c.build();
	const bytes = documents.reduce((sum, d) => sum + Buffer.byteLength(d.content, 'utf8'), 0);
	for (let i = 0; i < WARMUP; i++) run(documents, c.library);
	const durations: number[] = [];
	let count = 0;
	for (let i = 0; i < RUNS; i++) {
		const t0 = performance.now();
		count = run(documents, c.library);
		durations.push(performance.now() - t0);
	}
	const ms = median(durations);
	results.push({
		label: c.label,
		bytes,
		lines: documents.reduce((sum, d) => sum + d.content.split('\n').length, 0),
		extracted: count,
		ms: Number(ms.toFixed(2)),
		perSecond: count > 0 ? Math.round(count / (ms / 1000)) : null,
		mbPerSecond: Number((bytes / 1_048_576 / (ms / 1000)).toFixed(1)),
	});
	console.log(`${c.label.padEnd(28)} ${(bytes / 1_048_576).toFixed(2)} MB  ${String(count).padStart(7)}  ${ms.toFixed(2)} ms`);
}

const cpu = cpus()[0]?.model ?? 'unknown CPU';
await Bun.write(
	'benchmark-results.json',
	`${JSON.stringify({ host: `${cpu}, ${Math.round(totalmem() / 1_073_741_824)} GB RAM, Node ${process.versions.node}`, runs: RUNS, results }, null, 2)}\n`,
);
console.log('\nwrote benchmark-results.json');
