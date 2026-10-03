/**
 * Fails when the extension's audit drifts from the shared corpus, which is the
 * reason both frontends live in one repository at all.
 *
 * - fixtures/detection.json must reproduce under the port: parsing, locales,
 *   marks and whole audits, exactly as `crate/src/corpus.rs` checks them.
 * - fixtures/mcp-check-catalogues.json must reproduce under the npm server's
 *   tool, and — when the release binary is built — under the crate's server
 *   too, so the pinned cases are checked against both implementations rather
 *   than against a copy of the expected answers.
 *
 * Run: bun scripts/check-audit-parity.ts   (I18N_LE_BIN=<path> for the binary)
 */
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { isDeepStrictEqual } from 'node:util';
import { parseCatalogue } from '../src/audit/catalogue';
import type { LibraryId, Mark } from '../src/audit/library';
import { localesOf } from '../src/audit/locale';
import { keyMarks, marks } from '../src/audit/message';
import { reportFor } from '../src/audit/report';
import { TOOLS } from '../src/mcp/tools';

const ROOT = join(import.meta.dir, '..');
const CORPUS = join(ROOT, 'crate', 'fixtures');
const BINARY = process.env.I18N_LE_BIN ?? join(ROOT, 'crate', 'target', 'release', 'i18n-le');
const document = (name: string) => readFileSync(join(CORPUS, 'documents', name), 'utf8');
const failures: string[] = [];
const expectEqual = (label: string, actual: unknown, expected: unknown) => {
	if (!isDeepStrictEqual(JSON.parse(JSON.stringify(actual ?? null)), expected ?? null)) {
		failures.push(`${label}\n  got:      ${JSON.stringify(actual).slice(0, 400)}\n  expected: ${JSON.stringify(expected).slice(0, 400)}`);
	}
};

const detection = JSON.parse(readFileSync(join(CORPUS, 'detection.json'), 'utf8'));
for (const c of detection.parsing) {
	if (c.refused) {
		let threw = false;
		try {
			parseCatalogue(document(c.file));
		} catch {
			threw = true;
		}
		if (!threw) failures.push(`${c.file} parsed, and the corpus says it is refused`);
		continue;
	}
	const parsed = parseCatalogue(document(c.file));
	expectEqual(`${c.file} keys`, parsed.entries.map((e) => e.key), c.keys);
	expectEqual(`${c.file} duplicates`, parsed.duplicates, c.duplicates ?? []);
}
for (const c of detection.locales) {
	let locales: unknown = 'refused';
	try {
		locales = localesOf(c.names).map((l) => l ?? null);
	} catch {
		locales = 'refused';
	}
	expectEqual(`locales of ${c.names.join(', ')}`, locales, c.refused ? 'refused' : c.locales);
}
for (const c of detection.marks) {
	const parsed = parseCatalogue(document(c.file));
	const found: Mark[] = keyMarks(parsed.entries.map((e) => e.key));
	for (const e of parsed.entries) for (const m of e.text === undefined ? [] : marks(e.text)) if (!found.includes(m)) found.push(m);
	expectEqual(`marks of ${c.file}`, [...found].sort(), [...c.marks].sort());
}
for (const c of detection.audit) {
	const keysAreSource = c.options?.keysAreSource ?? false;
	const locales = localesOf(c.files);
	const report = reportFor(
		c.files.map((name: string, i: number) => ({ name, locale: locales[i], content: document(name) })),
		{ library: c.library as LibraryId, version: null, layout: null, keysAreSource, evidence: [] },
		{ source: c.options?.source, keysAreSource },
	);
	expectEqual(`audit: ${c.name}`, report, c.expected);
}

const mcp = JSON.parse(readFileSync(join(CORPUS, 'mcp-check-catalogues.json'), 'utf8'));
const argumentsOf = (c: { files?: string[]; arguments: Record<string, unknown> }) =>
	c.files ? { ...c.arguments, files: c.files.map((path) => ({ path, content: document(path) })) } : c.arguments;
const tool = TOOLS[0] as (typeof TOOLS)[number];
for (const c of mcp) {
	try {
		const answer = JSON.parse(JSON.stringify(await tool.handler(argumentsOf(c))));
		if (c.expectedError !== undefined) failures.push(`npm server answered "${c.name}", which the corpus refuses`);
		for (const [key, value] of Object.entries(c.expected ?? {})) expectEqual(`npm server: ${c.name} (${key})`, answer[key], value);
	} catch (error) {
		expectEqual(`npm server refusal: ${c.name}`, (error as Error).message, c.expectedError);
	}
}

let crateChecked = false;
if (existsSync(BINARY)) {
	crateChecked = true;
	const child = Bun.spawn([BINARY, 'mcp'], { stdin: 'pipe', stdout: 'pipe' });
	const out = new Response(child.stdout).text();
	child.stdin.write(
		`${mcp.map((c: never, id: number) => JSON.stringify({ jsonrpc: '2.0', id, method: 'tools/call', params: { name: 'check_catalogues', arguments: argumentsOf(c) } })).join('\n')}\n`,
	);
	child.stdin.end();
	for (const line of (await out).trim().split('\n')) {
		const response = JSON.parse(line);
		const c = mcp[response.id];
		if (response.result.isError) {
			expectEqual(`crate server refusal: ${c.name}`, response.result.content[0].text, c.expectedError);
			continue;
		}
		for (const [key, value] of Object.entries(c.expected ?? {})) {
			expectEqual(`crate server: ${c.name} (${key})`, response.result.structuredContent[key], value);
		}
	}
}

if (failures.length > 0) {
	console.error(`PARITY FAILED — ${failures.length} difference(s):\n`);
	for (const f of failures) console.error(`${f}\n`);
	process.exit(1);
}
console.log(
	`OK: every detection.json case reproduces under the port, and every mcp-check-catalogues.json case under the npm server${crateChecked ? ' and the crate server' : ' (no binary built, so the crate server was not asked)'}.`,
);
