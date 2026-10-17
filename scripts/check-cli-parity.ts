/**
 * Requires the npm command line to behave exactly as the Rust one does.
 *
 * Both install a command named `i18n-le`, so a person or a CI step cannot tell
 * which one they have. This is a different contract from
 * `check-extraction-parity.ts`, which holds the extension's engine equal to
 * the crate's: here the whole terminal surface is compared, identification,
 * the layout, the audit, the exit codes, stdout and stderr byte for byte,
 * and every reply `i18n-le mcp` writes, byte for byte too.
 *
 * Every case runs both binaries over one generated tree with stdout piped,
 * which is how a script sees them. `--version` is compared too: every surface
 * carries one version, and two binaries of one name disagreeing about it is
 * the first thing a person comparing them would see.
 *
 * Run: bun run build:cli && cargo build --locked --manifest-path crate/Cargo.toml
 *      bun scripts/check-cli-parity.ts
 */
import { spawnSync } from 'node:child_process';
import {
	chmodSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	rmdirSync,
	symlinkSync,
	unlinkSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const ROOT = join(import.meta.dir, '..');
const RUST =
	process.env.I18N_LE_BIN ??
	join(
		ROOT,
		'crate',
		'target',
		'debug',
		`i18n-le${process.platform === 'win32' ? '.exe' : ''}`,
	);
const NODE_CLI = join(ROOT, 'cli', 'cli.js');
/**
 * Node, by name, not `process.execPath`: this script runs under Bun, and the
 * package is run by Node wherever `npx` runs it. Bun's errors are not Node's.
 */
const NODE = process.env.NODE_BIN ?? 'node';

interface Case {
	readonly args: readonly string[];
	readonly stdin?: string | Buffer;
}

interface Outcome {
	readonly status: number | null;
	readonly stdout: string;
	readonly stderr: string;
}

for (const [label, path] of [
	['the Rust binary', RUST],
	['cli/cli.js', NODE_CLI],
] as const) {
	if (!existsSync(path)) {
		console.error(`FAIL: ${label} is missing at ${path}. Build it first.`);
		process.exit(1);
	}
}

const tree = mkdtempSync(join(tmpdir(), 'i18n-le-cli-parity-'));
const created: string[] = [];
const dirs: string[] = [tree];

function dir(relative: string): void {
	mkdirSync(join(tree, relative));
	dirs.push(join(tree, relative));
}

function file(relative: string, content: string | Buffer): void {
	writeFileSync(join(tree, relative), content);
	created.push(join(tree, relative));
}

function link(relative: string, target: string): void {
	// Windows grants symlinks to administrators and developer mode only, and
	// both binaries then see the same tree without the link.
	try {
		symlinkSync(target, join(tree, relative));
	} catch {
		return;
	}
	created.push(join(tree, relative));
}

const canLock = process.platform !== 'win32' && process.getuid?.() !== 0;

const json = (value: unknown) => `${JSON.stringify(value, null, 2)}\n`;

const EN = {
	greeting: 'Hello {{name}}',
	bye: 'Goodbye',
	count: '{{count}} items',
	nested: { title: 'Title', body: 'Body text' },
	same: 'OK',
};

function build(): void {
	// An i18next project: a manifest, a directory of <locale>.json.
	dir('i18next');
	file('i18next/package.json', json({ dependencies: { i18next: '^23.10.0' } }));
	dir('i18next/locales');
	file('i18next/locales/en.json', json(EN));
	file(
		'i18next/locales/de.json',
		json({ greeting: 'Hallo {{nom}}', bye: '', count: '{count} Dinge', nested: 'flach', same: 'OK', extra: 'x' }),
	);
	file('i18next/locales/fr.json', json({ greeting: 'Bonjour {{name}}', bye: 'Au revoir', nested: { title: 'Titre' } }));
	file('i18next/locales/es.json', '{"greeting": "Hola {{name}}", "greeting": "Hola de nuevo {{name}}", "bye": "Adiós"}\n');
	file('i18next/locales/it.json', '{"greeting": "Ciao {{name}}",\n  "bye": \n');
	file('i18next/locales/pt.json', `\uFEFF${json({ greeting: 'Olá {{name}}', bye: 'Tchau' })}`);
	file('i18next/locales/nl.json', '{\r\n  "greeting": "Hallo {{name}} %s",\r\n  "bye": "Doei {0}"\r\n}\r\n');
	if (canLock) {
		file('i18next/locales/sv.json', json({ greeting: 'Hej {{name}}' }));
		chmodSync(join(tree, 'i18next', 'locales', 'sv.json'), 0o000);
	}
	// next-intl: a manifest and single-brace messages.
	dir('next-intl');
	file('next-intl/package.json', json({ dependencies: { 'next-intl': '3.1.0', next: '14.0.0' } }));
	dir('next-intl/messages');
	file('next-intl/messages/en.json', json({ Home: { title: 'Welcome {name}', items: '{count, plural, one {# item} other {# items}}' } }));
	file('next-intl/messages/de.json', json({ Home: { title: 'Willkommen {nom}' } }));
	// A VS Code extension's bundles: keys are the English source.
	dir('vscode');
	file('vscode/package.json', json({ engines: { vscode: '^1.90.0' }, l10n: './l10n' }));
	dir('vscode/l10n');
	file('vscode/l10n/bundle.l10n.json', json({ 'Open {0}': 'Open {0}', Close: 'Close' }));
	file('vscode/l10n/bundle.l10n.de.json', json({ 'Open {0}': 'Öffnen', Close: 'Close' }));
	// Flutter: a pubspec and ARB files.
	dir('flutter');
	file('flutter/pubspec.yaml', 'name: app\ndependencies:\n  flutter_localizations:\n    sdk: flutter\n  intl: ^0.19.0\nflutter:\n  generate: true\n');
	file('flutter/l10n.yaml', 'arb-dir: lib/l10n\ntemplate-arb-file: app_en.arb\n');
	dir('flutter/lib');
	dir('flutter/lib/l10n');
	file('flutter/lib/l10n/app_en.arb', json({ '@@locale': 'en', hello: 'Hello {name}', '@hello': { placeholders: { name: {} } } }));
	file('flutter/lib/l10n/app_es.arb', json({ '@@locale': 'es', hello: 'Hola' }));
	// Nothing that says which library: a refusal listing what was found.
	dir('unknown');
	file('unknown/en.json', json({ a: 'A' }));
	file('unknown/de.json', json({ a: 'A' }));
	// Two libraries at once: a refusal naming both.
	dir('both');
	file('both/package.json', json({ dependencies: { i18next: '^23.0.0', 'next-intl': '^3.0.0' } }));
	dir('both/locales');
	file('both/locales/en.json', json({ a: 'Hi {{name}}', b: 'Hi {name}' }));
	file('both/locales/de.json', json({ a: 'Hallo {{name}}', b: 'Hallo {name}' }));
	// Only untranslated strings: info, which fails a run only when asked to.
	dir('untranslated');
	file('untranslated/package.json', json({ dependencies: { i18next: '^23.0.0' } }));
	dir('untranslated/locales');
	file('untranslated/locales/en.json', json({ a: 'Hello {{name}}', b: 'Bye' }));
	file('untranslated/locales/fr.json', json({ a: 'Hello {{name}}', b: 'Bye' }));
	// No catalogues at all.
	dir('empty');
	file('empty/package.json', json({ dependencies: { i18next: '^23.0.0' } }));
	dir('empty/locales');
}

function cases(): readonly Case[] {
	const at = (path: string) => join(tree, path);
	return [
		{ args: ['--help'] },
		{ args: ['-h'] },
		{ args: ['--version'] },
		{ args: ['-V'] },
		{ args: [at('i18next'), '--help'] },
		{ args: [] },
		{ args: [at('i18next')] },
		{ args: [at('i18next/locales')] },
		{ args: ['i18next/locales'] },
		{ args: ['./i18next/locales/'] },
		{ args: [at('i18next/locales'), '--strict'] },
		{ args: [at('i18next/locales'), '--fail-on', 'untranslated'] },
		{ args: [at('i18next/locales'), '--fail-on', 'any'] },
		{ args: [at('i18next/locales'), '--fail-on', 'some'] },
		{ args: [at('i18next/locales'), '--fail-on'] },
		{ args: [at('i18next/locales'), '--source', 'de'] },
		{ args: [at('i18next/locales'), '--source', at('i18next/locales/fr.json')] },
		{ args: [at('i18next/locales'), '--source', 'xx'] },
		{ args: [at('i18next/locales'), '--source'] },
		{ args: [at('i18next/locales'), '--keys-are-source'] },
		{ args: [at('i18next/locales'), '--system', 'next-intl'] },
		{ args: [at('i18next/locales'), '--system', 'Klingon'] },
		{ args: [at('i18next/locales'), '--system'] },
		{ args: [at('i18next/locales/en.json'), at('i18next/locales/de.json')] },
		{ args: [at('i18next/locales/en.json')] },
		{ args: [at('next-intl')] },
		{ args: [at('next-intl/messages')] },
		{ args: [at('vscode')] },
		{ args: [at('vscode/l10n'), '--keys-are-source'] },
		{ args: [at('flutter')] },
		{ args: [at('flutter/lib/l10n')] },
		{ args: [at('unknown')] },
		{ args: [at('unknown'), '--system', 'i18next'] },
		{ args: [at('both/locales')] },
		{ args: [at('both/locales'), '--system', 'i18next'] },
		{ args: [at('untranslated/locales')] },
		{ args: [at('untranslated/locales'), '--fail-on', 'untranslated'] },
		{ args: [at('untranslated/locales'), '--fail-on', 'any'] },
		{ args: [at('empty')] },
		{ args: [at('missing')] },
		{ args: [at('i18next/locales/nope.json')] },
		{ args: [at('i18next'), '--nope'] },
		{ args: [at('i18next'), '-'] },
		{ args: [join(ROOT, 'crate', 'fixtures', 'documents')] },
		{ args: [join(ROOT, 'crate', 'fixtures', 'documents'), '--system', 'i18next'] },
	];
}

function run(command: readonly string[], testCase: Case): Outcome {
	const [program, ...rest] = command as [string, ...string[]];
	const result = spawnSync(program, [...rest, ...testCase.args], {
		input: testCase.stdin ?? '',
		cwd: tree,
		maxBuffer: 256 * 1024 * 1024,
	});
	return {
		status: result.status,
		stdout: result.stdout.toString('utf8'),
		stderr: result.stderr.toString('utf8'),
	};
}

function compareCli(): string[] {
	const failures: string[] = [];
	for (const testCase of cases()) {
		const rust = run([RUST], testCase);
		const node = run([NODE, NODE_CLI], testCase);
		for (const field of ['status', 'stdout', 'stderr'] as const) {
			if (rust[field] === node[field]) continue;
			failures.push(
				`i18n-le ${testCase.args.join(' ')}\n  ${field} differs\n` +
					describeDifference(String(rust[field]), String(node[field])),
			);
		}
	}
	return failures;
}

function mcpLines(): readonly string[] {
	let id = 0;
	const request = (method: string, params?: unknown) =>
		JSON.stringify({
			jsonrpc: '2.0',
			id: ++id,
			method,
			...(params === undefined ? {} : { params }),
		});
	const call = (name: string, args?: unknown) =>
		request('tools/call', args === undefined ? { name } : { name, arguments: args });
	const check = (args: Record<string, unknown>) => call('check_catalogues', args);
	const files = [
		{ path: 'en.json', locale: 'en', content: json(EN) },
		{ path: 'de.json', locale: 'de', content: json({ greeting: 'Hallo {{nom}}', bye: '' }) },
	];
	return [
		request('initialize', { protocolVersion: '2024-11-05' }),
		JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }),
		request('tools/list'),
		request('ping'),
		request('resources/list'),
		request('tools/call'),
		request('tools/call', { arguments: {} }),
		request('tools/call', null),
		call('no_such_tool', {}),
		JSON.stringify({ jsonrpc: '2.0', id: 'text-id', method: 'ping' }),
		JSON.stringify({ jsonrpc: '2.0', id: null, method: 'ping' }),
		JSON.stringify({ id: 77, method: 'ping' }),
		JSON.stringify({ jsonrpc: '2.0', id: 78 }),
		'not json',
		'',
		'   ',
		'[1, 2]',
		`${request('ping')}\r`,
		check({ library: 'i18next', files }),
		check({ library: 'i18next', files, source: 'de' }),
		check({ library: 'i18next', files, maxResults: 1 }),
		check({ library: 'next-intl', files }),
		check({ library: 'klingon', files }),
		check({ library: 'i18next', files: [] }),
		check({ library: 'i18next', files: [{ path: 'en.json', content: '{"a": ' }] }),
		check({ library: 'i18next' }),
		check({}),
		call('check_catalogues', null),
	];
}

function serve(command: readonly string[], input: string | Buffer): Outcome {
	const [program, ...rest] = command as [string, ...string[]];
	const result = spawnSync(program, [...rest, 'mcp'], { input, cwd: tree });
	return {
		status: result.status,
		stdout: result.stdout.toString('utf8'),
		stderr: result.stderr.toString('utf8'),
	};
}

function compareMcp(): string[] {
	const failures: string[] = [];
	const lines = mcpLines();
	const sessions: readonly (readonly [string, string | Buffer])[] = [
		['the whole session', `${lines.join('\n')}\n`],
		['a last line with no newline', lines[3] as string],
		[
			'a line that is not UTF-8',
			Buffer.concat([
				Buffer.from(`${lines[3]}\n`),
				Buffer.from([0x7b, 0xff, 0x7d, 0x0a]),
				Buffer.from(`${lines[3]}\n`),
			]),
		],
	];
	for (const [label, input] of sessions) {
		const rust = serve([RUST], input);
		const node = serve([NODE, NODE_CLI], input);
		for (const field of ['status', 'stdout', 'stderr'] as const) {
			if (rust[field] === node[field]) continue;
			failures.push(
				`i18n-le mcp, ${label}\n  ${field} differs\n` +
					describeDifference(String(rust[field]), String(node[field])),
			);
		}
	}
	return failures;
}

/** The first differing line, which is the one worth reading in a long output. */
function describeDifference(rust: string, node: string): string {
	const left = rust.split('\n');
	const right = node.split('\n');
	const index = left.findIndex((line, at) => line !== right[at]);
	const at = index === -1 ? left.length : index;
	const clip = (line: string | undefined) =>
		line === undefined || line.length <= 2000 ? line : `${line.slice(0, 2000)}…`;
	return `  line ${at + 1}\n  rust: ${clip(left[at])}\n  node: ${clip(right[at])}`;
}

function cleanUp(): void {
	const unlock: readonly (readonly [string, number])[] = [
		[join(tree, 'i18next', 'locales', 'sv.json'), 0o600],
	];
	for (const [path, mode] of unlock) if (existsSync(path)) chmodSync(path, mode);
	for (const path of created) unlinkSync(path);
	for (const path of [...dirs].reverse()) rmdirSync(path);
}

build();
let failures: string[];
try {
	failures = [...compareCli(), ...compareMcp()];
} finally {
	cleanUp();
}

if (failures.length > 0) {
	console.error(failures.join('\n\n'));
	console.error(`\nFAIL: ${failures.length} difference(s) between the npm and Rust CLIs`);
	process.exit(1);
}
console.log(
	`OK: the npm CLI matches the Rust CLI on ${cases().length} commands and ${mcpLines().length} MCP lines`,
);
