/**
 * `check_catalogues` is offered by BOTH servers — the npm one in
 * `src/mcp/tools.ts` and the Rust one in `crate/src/mcp/check.rs`. One tool
 * name, one schema, two implementations. An agent asking that tool must get the
 * same answer whichever server it reaches, so the contract is identical output,
 * not similar output.
 *
 * `crate/fixtures/mcp-check-catalogues.json` pins that over cases somebody
 * thought of. This generates them instead: catalogue sets in every placeholder
 * style, with plural and metadata keys, duplicates, nested and flat shapes,
 * whitespace only Rust calls whitespace — and catalogues broken at random
 * bytes, because a file that will not parse is reported with serde_json's own
 * message, line and byte column, and the npm reader has to say exactly the
 * same thing.
 *
 * What is NOT compared: the CLI against the extension. Those are two surfaces
 * with two jobs, and only the shared tool is held to identity.
 *
 * Run: bun scripts/check-audit-differential.ts
 *   I18N_LE_DIFFERENTIAL_SEED=<n>  reproduce a specific failure
 *   I18N_LE_DIFFERENTIAL_CASES=<n> how many sets (default 1500)
 *   I18N_LE_BIN=<path>             the Rust binary (default the release build)
 */
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { TOOLS } from '../src/mcp/tools';

const ROOT = join(import.meta.dir, '..');
const BINARY = process.env.I18N_LE_BIN ?? join(ROOT, 'crate', 'target', 'release', 'i18n-le');
const SEED = Number(process.env.I18N_LE_DIFFERENTIAL_SEED ?? 20261003);
const CASES = Number(process.env.I18N_LE_DIFFERENTIAL_CASES ?? 1500);

/** Mulberry32: a named, seeded source, so a failing run reproduces from one number. */
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

const LIBRARIES = ['i18next', 'next-intl', 'vscode-l10n', 'flutter-arb'];

const KEYS: readonly string[] = [
	'title', 'greeting', 'nav', 'home', 'settings', 'item_one', 'item_other', 'item_few', 'step_one',
	'count_zero', 'a.b', 'a', 'b', '@@locale', '@title', '@greeting', '@@x', 'Save file', 'Open {0} files',
	'Délai dépassé', 'ключ', '键', 'x y z', '', 'end',
];

const PIECES: readonly string[] = [
	'Hello', 'Bonjour', 'Привет', '你好', 'مرحبا', ' ', '', '{{name}}', '{{ name }}', '{{user.name}}',
	'{{}}', '{{ 1x }}', '{name}', '{ name }', '{0}', '{12}', '{count, plural, one {# item} other {# items}}',
	'{n,select,a{x}b{y}}', '{count, plural, one {', '%s', '%d', '%1$s', '%-os', '90%-os', '100%% off', '%',
	'${x}', '$t(other.key)', '$t(', '$', '{ $var }', '{$x}', '}}', '{{', '}', '{', '\\n', '\\t', '\\"',
	'\\\\', '\\u0041', '\\u00e9', '\\ud83d\\ude00', '\u0085', '﻿', '　', ' ', '\t', 'é', '😀',
];

/** Values that are not strings: each is `other` to the reader. */
const NON_STRINGS: readonly string[] = [
	'12', '-0', '0.5', '1e3', '1E+2', '-1.5e-3', '18446744073709551615', '18446744073709551616',
	'123456789012345678901234567890', '1e308', '1.7976931348623157e308', '0.000001e-400', 'true', 'false',
	'null', '[]', '[1, "a", {"b": [null]}]', '[[[]]]',
];

/** Corruptions, each a way a real catalogue breaks — and each a serde_json message. */
const BREAKERS: ReadonlyArray<(text: string, random: () => number) => string> = [
	(t, r) => t.slice(0, Math.floor(r() * t.length)),
	(t, r) => {
		const at = Math.floor(r() * t.length);
		return t.slice(0, at) + pickFrom(r, ['}', ']', ',', ':', '"', '\\', '{', '[', 'x', '\u0001', '\n', 'é']) + t.slice(at);
	},
	(t, r) => {
		const at = Math.floor(r() * t.length);
		return t.slice(0, at) + t.slice(at + 1);
	},
	(t) => t.replace(/}\s*$/, ',}'),
	(t) => `${t} trailing`,
	(t) => afterBrace(t, '{"n": 1e400, '),
	(t) => afterBrace(t, '{"n": 01, '),
	(t) => afterBrace(t, '{"n": 1., '),
	(t) => afterBrace(t, '{"n": -, '),
	(t) => afterBrace(t, '{"n": 1e, '),
	(t) => afterBrace(t, '{"s": "\\ud800", '),
	(t) => afterBrace(t, '{"s": "\\udc00", '),
	(t) => afterBrace(t, '{"s": "\\ud800\\u0041", '),
	(t) => afterBrace(t, '{"s": "\\ud800x", '),
	(t) => afterBrace(t, '{"s": "\\x", '),
	(t) => afterBrace(t, '{"s": "\\u12G4", '),
	(t) => afterBrace(t, '{"s": "\\u12'),
	(t) => afterBrace(t, '{"s": "tab\there", '),
	(t) => afterBrace(t, '{"t": tru, '),
	(t) => afterBrace(t, '{"t": nul'),
	(t) => afterBrace(t, '{1: "x", '),
	(t) => afterBrace(t, '{"a" "x", '),
	(t) => afterBrace(t, `{"deep": ${'{"d":'.repeat(130)}1${'}'.repeat(130)}, `),
	(t) => afterBrace(t, `{"deep": ${'{"d":'.repeat(126)}1${'}'.repeat(126)}, `),
	(t) => afterBrace(t, `{"list": ${'['.repeat(200)}${']'.repeat(200)}, `),
	(t) => afterBrace(t, '{"list": [1, 2,], '),
	(t) => afterBrace(t, '{"list": [1 2], '),
	(t) => afterBrace(t, '{"list": [{"a" 1}], '),
	(t) => afterBrace(t, '{"list": [{"a": 1,}], '),
	(t) => afterBrace(t, '{"list": [{1: 2}], '),
	() => '[]',
	() => '"text"',
	() => '',
	() => '   ',
];

/**
 * Insert text just after the document's opening brace. Every generated
 * catalogue starts with `{`, so this adds a member (or a defect) at the front.
 */
function afterBrace(text: string, inserted: string): string {
	return `${text.slice(0, 1)}${inserted}${text.slice(1)}`;
}

function pickFrom<T>(random: () => number, list: readonly T[]): T {
	return list[Math.floor(random() * list.length)] as T;
}

const PREFIXES: readonly string[] = ['', '', 'messages.', 'bundle.l10n.', 'package.nls.', 'app_', 'i18n.'];
const TAGS: readonly string[] = [
	'en', 'de', 'es', 'fr', 'pt-BR', 'pt_br', 'pt-br', 'zh-Hans-CN', 'zh_CN', 'ja', 'pl', 'en-GB', 'sr-Latn',
	'es-419', 'de', 'fr', 'ja', 'Spanish', 'nls',
];

interface Generated {
	readonly name: string;
	readonly args: Record<string, unknown>;
}

function generate(count: number, seed: number): Generated[] {
	const random = seeded(seed);
	const pick = <T>(list: readonly T[]): T => pickFrom(random, list);

	const message = (): string =>
		Array.from({ length: Math.floor(random() * 4) }, () => pick(PIECES)).join(pick(['', ' ']));

	const object = (depth: number): string => {
		const members: string[] = [];
		const size = Math.floor(random() * 6);
		for (let k = 0; k < size; k++) {
			const key = JSON.stringify(pick(KEYS)).slice(1, -1);
			const roll = random();
			let value: string;
			// The pieces are JSON source already — their escapes are meant to be read
			// by the parser — so the message is quoted, never re-escaped.
			if (roll < 0.62) value = `"${message()}"`;
			else if (roll < 0.78 && depth < 3) value = object(depth + 1);
			else value = pick(NON_STRINGS);
			members.push(`"${key}": ${value}`);
		}
		const pretty = random() < 0.5;
		return pretty ? `{\n${members.map((m) => `  ${m}`).join(',\n')}\n}` : `{${members.join(',')}}`;
	};

	const out: Generated[] = [];
	for (let index = 0; index < count; index++) {
		const library = pick(LIBRARIES);
		const prefix = pick(PREFIXES);
		const extension = library === 'flutter-arb' && random() < 0.7 ? '.arb' : '.json';
		const size = 1 + Math.floor(random() * 4);
		// A base catalogue needs a prefix to be told apart: `messages.json` beside
		// `messages.de.json`, never a bare `.json`.
		const base = prefix !== '' && random() < 0.3;
		const supplyLocales = random() < 0.2;

		const files: Array<Record<string, unknown>> = [];
		const used = new Set<string>();
		for (let k = 0; k < size; k++) {
			// Most sets carry exactly one English candidate, as real ones do; the rest
			// exercise the refusals.
			const english = k === 0 && random() < 0.85;
			const tag = english ? (base ? '' : pick(['en', 'en-GB', 'en_US'])) : pick(TAGS.filter((t) => !t.startsWith('en')));
			let path = `${prefix}${tag}`.replace(/[._]$/, '');
			if (path === '') path = 'base';
			path += extension;
			if (used.has(path)) continue;
			used.add(path);
			let content = object(0);
			// A break can land inside a surrogate pair. A request carrying a lone one is
			// not JSON to serde_json, so the crate drops the frame unanswered: no client
			// can ask that question, so the generator never does.
			if (random() < 0.12) content = pick(BREAKERS)(content, random).toWellFormed();
			if (random() < 0.08) content = `﻿${content}`;
			if (random() < 0.08) content = content.replace(/\n/g, '\r\n');
			const file: Record<string, unknown> = { path, content };
			if (supplyLocales) file.locale = tag === '' ? 'en' : tag;
			files.push(file);
		}

		const args: Record<string, unknown> = { library, files };
		if (random() < 0.15) args.keysAreSource = true;
		const sourceRoll = random();
		if (sourceRoll < 0.1) args.source = (files[0] as { path: string }).path;
		else if (sourceRoll < 0.18) args.source = pick(TAGS);
		if (random() < 0.08) args.maxResults = 1 + Math.floor(random() * 3);
		out.push({ name: `${index}:${library}:${files.map((f) => f.path).join(',')}`, args });
	}
	return out;
}

/** JSON with object keys in a fixed order, so "identical" means identical. */
function canonical(value: unknown): string {
	if (value === null || typeof value !== 'object') return JSON.stringify(value);
	if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
	const entries = Object.entries(value as Record<string, unknown>)
		.filter(([, item]) => item !== undefined)
		.sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
	return `{${entries.map(([k, v]) => `${JSON.stringify(k)}:${canonical(v)}`).join(',')}}`;
}

function show(text: string): string {
	return text.length > 900 ? `${text.slice(0, 900)}…` : text;
}

async function fromNpm(documents: readonly Generated[]): Promise<string[]> {
	const tool = TOOLS.find((candidate) => candidate.name === 'check_catalogues');
	if (!tool) throw new Error('the npm server no longer offers check_catalogues');
	const answers: string[] = [];
	for (const document of documents) {
		try {
			answers.push(canonical(JSON.parse(JSON.stringify(await tool.handler(document.args)))));
		} catch (error) {
			answers.push(`error: ${error instanceof Error ? error.message : String(error)}`);
		}
	}
	return answers;
}

async function fromCrate(documents: readonly Generated[]): Promise<string[]> {
	if (!existsSync(BINARY)) {
		throw new Error(`no binary at ${BINARY} — build it first: cd crate && cargo build --release`);
	}
	const child = Bun.spawn([BINARY, 'mcp'], { stdin: 'pipe', stdout: 'pipe', stderr: 'pipe' });
	const draining = new Response(child.stdout).text();
	const requests = documents
		.map((document, id) =>
			JSON.stringify({
				jsonrpc: '2.0',
				id,
				method: 'tools/call',
				params: { name: 'check_catalogues', arguments: document.args },
			}),
		)
		.join('\n');
	child.stdin.write(`${requests}\n`);
	child.stdin.end();
	const stdout = await draining;
	await child.exited;

	const answers: string[] = new Array(documents.length);
	for (const line of stdout.split('\n')) {
		if (line.trim().length === 0) continue;
		const response = JSON.parse(line) as {
			id: number;
			result?: { structuredContent?: unknown; isError?: boolean; content?: { text: string }[] };
			error?: unknown;
		};
		if (response.error !== undefined) {
			throw new Error(`the crate server refused set ${response.id}: ${JSON.stringify(response.error)}`);
		}
		answers[response.id] = response.result?.isError
			? `error: ${response.result.content?.[0]?.text}`
			: canonical(response.result?.structuredContent);
	}
	const missing = answers.findIndex((answer) => answer === undefined);
	if (missing !== -1) {
		throw new Error(`the crate server never answered set ${missing}: ${await new Response(child.stderr).text()}`);
	}
	return answers;
}

const documents = generate(CASES, SEED);
console.log(`differential: ${documents.length} generated catalogue sets, seed ${SEED}, binary ${BINARY.replace(ROOT, '.')}`);

const [npm, crate] = await Promise.all([fromNpm(documents), fromCrate(documents)]);
const failures: string[] = [];
const outcomes = new Map<string, number>();
for (const [index, document] of documents.entries()) {
	const ours = npm[index] as string;
	const theirs = crate[index] as string;
	const outcome = ours.startsWith('error:')
		? `refused (${ours.replace(/^error: /, '').split(/[\s:]/).slice(0, 3).join(' ')})`
		: (JSON.parse(ours).data.status as string);
	outcomes.set(outcome, (outcomes.get(outcome) ?? 0) + 1);
	if (ours.includes('"code":"unparsable"')) outcomes.set('with an unparsable file', (outcomes.get('with an unparsable file') ?? 0) + 1);
	if (ours !== theirs) {
		failures.push(
			`the two check_catalogues servers disagree on "${document.name}"\n` +
				`  arguments: ${show(JSON.stringify(document.args))}\n` +
				`  npm:   ${show(ours)}\n` +
				`  crate: ${show(theirs)}`,
		);
	}
}

console.log(`  ${[...outcomes].map(([outcome, n]) => `${n} ${outcome}`).join(', ')}`);
if (failures.length > 0) {
	console.error(`\nDIFFERENTIAL FAILED — ${failures.length} problem(s):\n`);
	for (const failure of failures.slice(0, 10)) console.error(`${failure}\n`);
	if (failures.length > 10) console.error(`…and ${failures.length - 10} more.`);
	process.exit(1);
}
console.log('OK: both check_catalogues servers gave identical answers on every set.');
