/**
 * The extension identifies a project's i18n library and reads its catalogue
 * set with a port of the crate's `identify.rs` and `layout.rs`. This checks the
 * port, not the surfaces: it builds real project trees — manifests, configs,
 * call sites, catalogue directories in every layout — runs the crate's CLI and
 * the extension's engine over each, and compares the report, or the refusal.
 *
 * The one place the two are allowed to differ is how a refusal tells the reader
 * to name the library: the CLI names its flag and the extension its setting.
 * The engine takes that phrase as an argument, so this passes the CLI's.
 *
 * Every file it writes it removes again, one path at a time, and then the
 * directories it made, innermost first. Nothing is removed recursively.
 *
 * Run: bun scripts/check-identify-port.ts
 *   I18N_LE_PORT_SEED=<n>   reproduce a specific failure
 *   I18N_LE_PORT_CASES=<n>  how many trees (default 300)
 *   I18N_LE_BIN=<path>      the Rust binary (default the release build)
 */
import { spawnSync } from 'node:child_process';
import {
	existsSync,
	mkdirSync,
	mkdtempSync,
	readdirSync,
	readFileSync,
	realpathSync,
	rmdirSync,
	statSync,
	unlinkSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import type { EntryType, FileSystem } from '../src/audit/fs';
import { scan } from '../src/audit/scan';

const ROOT = join(import.meta.dir, '..');
const BINARY = process.env.I18N_LE_BIN ?? join(ROOT, 'crate', 'target', 'release', 'i18n-le');
const SEED = Number(process.env.I18N_LE_PORT_SEED ?? 20261003);
const CASES = Number(process.env.I18N_LE_PORT_CASES ?? 300);
const CLI_HOW_TO_NAME = 'with --system <name>';

function seeded(seed: number): () => number {
	let state = seed >>> 0;
	return () => {
		state = (state + 0x6d2b79f5) >>> 0;
		let t = state;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

/** The engine's filesystem, over `node:fs`. */
const nodeFs: FileSystem = {
	async readDirectory(path) {
		try {
			return readdirSync(path, { withFileTypes: true }).map((entry): readonly [string, EntryType] => [
				entry.name,
				entry.isDirectory() ? 'directory' : entry.isFile() ? 'file' : 'other',
			]);
		} catch {
			// Unreadable is empty, as `read_dir(...).into_iter().flatten()` reads it.
			return [];
		}
	},
	async stat(path) {
		try {
			const stats = statSync(path);
			return { type: stats.isDirectory() ? 'directory' : stats.isFile() ? 'file' : 'other', size: stats.size };
		} catch {
			// Nothing there.
			return undefined;
		}
	},
	async readFile(path) {
		return readFileSync(path);
	},
};

const MESSAGES: readonly string[] = [
	'Hello', 'Hello {{name}}', 'Hello {name}', 'Open {0} files', '{count, plural, one {# item} other {# items}}',
	'$t(common.title)', '%s left', '', 'Hola', '{{ count }} new',
];

interface Tree {
	readonly root: string;
	readonly files: string[];
	readonly directories: string[];
}

function writeTree(base: string, layout: Record<string, string>): Tree {
	const tree: Tree = { root: base, files: [], directories: [] };
	for (const [relative, content] of Object.entries(layout)) {
		const path = join(base, relative);
		let directory = dirname(path);
		const made: string[] = [];
		while (!existsSync(directory)) {
			made.unshift(directory);
			directory = dirname(directory);
		}
		for (const dir of made) {
			mkdirSync(dir);
			tree.directories.push(dir);
		}
		writeFileSync(path, content);
		tree.files.push(path);
	}
	return tree;
}

function removeTree(tree: Tree): void {
	for (const file of tree.files) unlinkSync(file);
	for (const directory of [...tree.directories].reverse()) rmdirSync(directory);
}

function generate(random: () => number): { layout: Record<string, string>; target: string; named?: string } {
	const pick = <T>(list: readonly T[]): T => list[Math.floor(random() * list.length)] as T;
	const catalogue = (keys: readonly string[], arb: boolean, locale: string): string => {
		const members = keys.map((key) => `${JSON.stringify(key)}: ${JSON.stringify(pick(MESSAGES))}`);
		if (arb) {
			members.unshift(`"@@locale": ${JSON.stringify(locale)}`);
			if (random() < 0.8) members.push(`"@${keys[0]}": {"description": "x"}`);
		}
		if (random() < 0.05) return '{ broken';
		return `{\n  ${members.join(',\n  ')}\n}\n`;
	};
	const keys = pick([
		['title', 'greeting'],
		['item_one', 'item_other', 'title'],
		['Save file', 'Open {0} files', 'Close'],
		['nav.home', 'nav.settings'],
	]);

	const layout: Record<string, string> = {};
	const deps: Record<string, string> = {};
	const dep = pick(['i18next', 'i18next@22', 'react-i18next', 'next-intl', '@vscode/l10n', 'none', 'none', 'two']);
	if (dep === 'i18next') deps.i18next = '^26.2.0';
	if (dep === 'i18next@22') deps.i18next = '^22.0.0';
	if (dep === 'react-i18next') deps['react-i18next'] = '^15.0.0';
	if (dep === 'next-intl') deps['next-intl'] = '^4.0.0';
	if (dep === '@vscode/l10n') deps['@vscode/l10n'] = '^0.0.18';
	if (dep === 'two') Object.assign(deps, { i18next: '^26.0.0', 'next-intl': '^4.0.0' });
	const manifestRoll = random();
	if (manifestRoll < 0.75) {
		const manifest: Record<string, unknown> = { name: 'app', [pick(['dependencies', 'devDependencies'])]: deps };
		if (random() < 0.15) manifest.l10n = './l10n';
		layout['package.json'] = random() < 0.05 ? '{ "name": ' : JSON.stringify(manifest, null, 2);
	} else if (manifestRoll < 0.9) {
		layout['pubspec.yaml'] = `name: app\ndependencies:\n  flutter:\n    sdk: flutter\n  flutter_localizations:\n    sdk: flutter\n  intl: ^0.19.0\n`;
	}
	const config = pick(['', '', 'i18next-parser.config.js', 'next-intl.config.ts', 'l10n.yaml', 'l10n.json']);
	if (config) layout[config] = config.endsWith('.json') ? '{}' : 'export default {}\n';
	const call = pick(['', 'useTranslation()', 'useTranslations()', 'vscode.l10n.t("x")', 'AppLocalizations.of(context)']);
	if (call) layout['src/app.ts'] = `const t = ${call};\n`;

	const shape = pick(['shared', 'shared', 'arb', 'bundle', 'nls', 'namespaced', 'namespaced-two']);
	const locales = pick([['en', 'de'], ['en', 'pt-BR', 'ja'], ['de', 'fr'], ['en-GB', 'en-US', 'de']]);
	let target = '';
	if (shape === 'shared') {
		const directory = pick(['locales', 'messages', 'translations', 'i18n']);
		for (const locale of locales) layout[`${directory}/${locale}.json`] = catalogue(keys, false, locale);
		target = directory;
	}
	if (shape === 'arb') {
		for (const locale of locales) layout[`lib/l10n/app_${locale.replace('-', '_')}.arb`] = catalogue(keys, true, locale);
		target = 'lib/l10n';
	}
	if (shape === 'bundle') {
		layout['l10n/bundle.l10n.json'] = catalogue(keys, false, 'en');
		for (const locale of locales.slice(1)) layout[`l10n/bundle.l10n.${locale.toLowerCase()}.json`] = catalogue(keys, false, locale);
		target = 'l10n';
	}
	if (shape === 'nls') {
		layout['package.nls.json'] = catalogue(keys, false, 'en');
		for (const locale of locales.slice(1)) layout[`src/i18n/package.nls.${locale.toLowerCase()}.json`] = catalogue(keys, false, locale);
		target = 'src/i18n';
	}
	if (shape.startsWith('namespaced')) {
		for (const locale of locales) {
			layout[`locales/${locale}/common.json`] = catalogue(keys, false, locale);
			if (shape === 'namespaced-two') layout[`locales/${locale}/errors.json`] = catalogue(keys, false, locale);
		}
		target = 'locales';
	}
	const named = random() < 0.15 ? pick(['i18next', 'next-intl', 'vscode-l10n', 'flutter-arb']) : undefined;
	return named === undefined ? { layout, target } : { layout, target, named };
}

function canonical(value: unknown): string {
	if (value === null || typeof value !== 'object') return JSON.stringify(value);
	if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
	const entries = Object.entries(value as Record<string, unknown>)
		.filter(([, item]) => item !== undefined)
		.sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
	return `{${entries.map(([k, v]) => `${JSON.stringify(k)}:${canonical(v)}`).join(',')}}`;
}

if (!existsSync(BINARY)) throw new Error(`no binary at ${BINARY} — build it first: cd crate && cargo build --release`);
const random = seeded(SEED);
const scratch = realpathSync(mkdtempSync(join(tmpdir(), 'i18n-le-port-')));
const failures: string[] = [];
const outcomes = new Map<string, number>();

try {
	for (let index = 0; index < CASES; index++) {
		const { layout, target, named } = generate(random);
		const base = join(scratch, `case-${index}`);
		mkdirSync(base);
		const tree = writeTree(base, layout);
		try {
			const input = join(base, target);
			const args = named === undefined ? [input] : ['--system', named, input];
			const cli = spawnSync(BINARY, args, { encoding: 'utf8' });
			const theirs =
				cli.status === 2 ? `refused: ${cli.stderr.trim().replace(/^i18n-le: /, '')}` : canonical(JSON.parse(cli.stdout));

			let ours: string;
			try {
				const report = await scan({
					fs: nodeFs,
					inputs: [input],
					named: named as never,
					source: undefined,
					keysAreSource: false,
					howToName: CLI_HOW_TO_NAME,
				});
				ours = canonical(report);
			} catch (error) {
				ours = `refused: ${error instanceof Error ? error.message : String(error)}`;
			}
			const outcome = ours.startsWith('refused') ? 'refused' : (JSON.parse(ours).status as string);
			outcomes.set(outcome, (outcomes.get(outcome) ?? 0) + 1);
			if (ours !== theirs) {
				failures.push(
					`case ${index} (${target}${named ? `, --system ${named}` : ''}) disagrees\n  tree: ${JSON.stringify(Object.keys(layout))}\n  engine: ${ours.slice(0, 900)}\n  cli:    ${theirs.slice(0, 900)}`,
				);
			}
		} finally {
			removeTree(tree);
			rmdirSync(base);
		}
	}
} finally {
	rmdirSync(scratch);
}

console.log(`identify port: ${CASES} generated project trees, seed ${SEED}`);
console.log(`  ${[...outcomes].map(([outcome, n]) => `${n} ${outcome}`).join(', ')}`);
if (failures.length > 0) {
	console.error(`\nPORT CHECK FAILED — ${failures.length} tree(s):\n`);
	for (const failure of failures.slice(0, 8)) console.error(`${failure}\n`);
	process.exit(1);
}
console.log('OK: the engine identified and read every tree exactly as the crate did.');
