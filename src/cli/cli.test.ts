import {
	mkdirSync,
	mkdtempSync,
	rmdirSync,
	unlinkSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Readable } from 'node:stream';
import { afterAll, describe, expect, it } from 'vitest';
import type { Finding } from '../audit/audit';
import type { Report } from '../audit/report';
import { parse } from './args';
import { serve } from './mcp';
import { exitCode, describe as line, run } from './run';
import { FLAGS, USAGE } from './usage';

describe('usage', () => {
	it('names every flag the parser accepts, and no other', () => {
		const named = new Set(
			USAGE.split(/\s+/)
				.filter((word) => word.startsWith('--'))
				.map((word) => word.replace(/[,.:;]+$/, ''))
				.filter((word) => word !== '--version' && word !== '--help'),
		);
		expect([...named].sort()).toEqual([...FLAGS].sort());
	});
});

describe('parse', () => {
	it('refuses what the crate refuses, in its words', () => {
		expect(() => parse(['--system'])).toThrow(
			'--system needs one of i18next, next-intl, vscode-l10n, flutter-arb',
		);
		expect(() => parse(['--system', 'Klingon', 'x'])).toThrow(
			'Klingon is not a library this reads. Try one of',
		);
		expect(() => parse(['--fail-on', 'some', 'x'])).toThrow(
			'--fail-on takes untranslated or any, not some',
		);
		expect(() => parse(['--source'])).toThrow(
			'--source needs a path or a language tag',
		);
		expect(() => parse(['-x'])).toThrow('-x is not an option. Try --help.');
		expect(() => parse([])).toThrow(
			'name the directory holding the catalogues',
		);
	});
});

describe('exitCode', () => {
	const report = (severities: Finding['severity'][], diagnostics = 0) =>
		({
			findings: severities.map((severity) => ({ severity })),
			diagnostics: Array.from({ length: diagnostics }, () => ({})),
		}) as unknown as Report;
	const cli = (failOn: 'findings' | 'untranslated' | 'any', strict = false) =>
		({ failOn, strict }) as never;

	it('leaves an untranslated string out of the exit code unless asked', () => {
		expect(exitCode(report(['info']), cli('findings'))).toBe(0);
		expect(exitCode(report(['info']), cli('untranslated'))).toBe(1);
		expect(exitCode(report(['warning']), cli('findings'))).toBe(1);
	});

	it('fails an unread catalogue only under --strict', () => {
		expect(exitCode(report([], 1), cli('findings'))).toBe(0);
		expect(exitCode(report([], 1), cli('findings', true))).toBe(2);
	});
});

describe('describe', () => {
	const finding = (rest: Record<string, unknown>) =>
		({
			file: 'de.json',
			key: 'k',
			severity: 'error',
			...rest,
		}) as unknown as Finding;

	it('projects each kind of evidence as the crate words it', () => {
		expect(
			line(
				finding({
					kind: 'placeholder-name-mismatch',
					sourceTokens: ['a'],
					targetTokens: ['b'],
				}),
			),
		).toBe('de.json: placeholder renamed k (a became b)');
		expect(
			line(
				finding({
					kind: 'placeholder-count-mismatch',
					sourceCount: 1,
					targetCount: 0,
				}),
			),
		).toBe('de.json: placeholder count k (1 placeholder became 0)');
		expect(
			line(
				finding({
					kind: 'structure-mismatch',
					sourceShape: 'object',
					targetShape: 'text',
				}),
			),
		).toBe('de.json: structure k (a string here, an object in the source)');
		expect(
			line(finding({ kind: 'duplicate-key-within-file', occurrences: 2 })),
		).toBe('de.json: duplicate k (defined 2×)');
		expect(line(finding({ kind: 'missing-key' }))).toBe('de.json: missing k');
	});
});

describe('run', () => {
	const root = mkdtempSync(join(tmpdir(), 'i18n-le-run-'));
	mkdirSync(join(root, 'locales'));
	const files = {
		'package.json': '{"dependencies": {"i18next": "^23.0.0"}}\n',
		'locales/en.json': '{"greeting": "Hello {{name}}", "bye": "Bye"}\n',
		'locales/de.json': '{"greeting": "Hallo {{nom}}"}\n',
	};
	for (const [name, content] of Object.entries(files))
		writeFileSync(join(root, name), content);

	afterAll(() => {
		for (const name of Object.keys(files)) unlinkSync(join(root, name));
		rmdirSync(join(root, 'locales'));
		rmdirSync(root);
	});

	const io = () => {
		const out: string[] = [];
		const err: string[] = [];
		return {
			out,
			err,
			io: {
				stdout: (text: string) => out.push(text),
				stderr: (text: string) => err.push(text),
				stdoutIsTerminal: false,
				version: '9.9.9',
			},
		};
	};

	it('audits a project and quotes no translation', async () => {
		const { out, err, io: streams } = io();
		expect(await run([join(root, 'locales')], streams)).toBe(1);
		expect(`${out.join('')}${err.join('')}`).not.toContain('Hallo');
		expect(err.join('').split('\n').slice(1, 4)).toEqual([
			'de.json: missing bye',
			'de.json: placeholder renamed greeting (name became nom)',
			'2 findings across 2 catalogues',
		]);
	});

	it('names a missing first path as typed, in Rust words', async () => {
		const { err, io: streams } = io();
		expect(await run(['no-such-dir-here'], streams)).toBe(2);
		// The OS words the error; the parity check compares that wording per OS.
		expect(err.join('')).toMatch(
			/^i18n-le: no-such-dir-here: [^\n]+ \(os error 2\)\n$/,
		);
	});
});

describe('mcp', () => {
	it('offers the one shared tool, keys sorted', async () => {
		const written: string[] = [];
		const request = { jsonrpc: '2.0', id: 1, method: 'tools/list' };
		await serve(
			Readable.from([Buffer.from(`${JSON.stringify(request)}\n`)]),
			(reply) => {
				written.push(reply);
				return undefined;
			},
			'9.9.9',
		);
		const reply = JSON.parse(written[0] ?? '{}');
		expect(
			reply.result.tools.map((tool: { name: string }) => tool.name),
		).toEqual(['check_catalogues']);
		expect(written[0]?.startsWith('{"id":1,"jsonrpc":"2.0","result":')).toBe(
			true,
		);
	});
});

describe('mcp: the crate framing', () => {
	// The protocol around the tools answers as the extension's server and
	// the crate's answer, so a client cannot tell the three apart by what it
	// sent. Replies are parsed: key order is the crate's concern, not this one.
	async function session(
		lines: readonly (string | Uint8Array)[],
	): Promise<{ status: number; replies: unknown[] }> {
		const written: string[] = [];
		const input = Readable.from(lines.map((item) => Buffer.from(item)));
		const status = await serve(
			input,
			(line) => {
				written.push(line);
				return undefined;
			},
			'9.9.9',
		);
		return { status, replies: written.map((line) => JSON.parse(line)) };
	}

	it("answers a malformed frame in the extension server's words and serves the next line", async () => {
		// A frame that is not JSON, or not UTF-8, was dropped without a word,
		// and a client lost every request it then waited on.
		const { status, replies } = await session([
			'{"jsonrpc":"2.0","id":1,"method":"initialize"}\n',
			'{"jsonrpc":"2.0","method":"notifications/initialized"}\n',
			'not json\n',
			Uint8Array.from([0x7b, 0xff, 0x7d, 0x0a]),
			'   \n',
			'{"jsonrpc":"2.0","id":2,"method":"nope"}\n',
		]);
		expect(status).toBe(0);
		expect(replies).toEqual([
			{
				jsonrpc: '2.0',
				id: 1,
				result: {
					protocolVersion: '2025-06-18',
					capabilities: { tools: {} },
					serverInfo: { name: 'i18n-le', version: '9.9.9' },
				},
			},
			{
				jsonrpc: '2.0',
				id: null,
				error: { code: -32700, message: 'Invalid JSON' },
			},
			{
				jsonrpc: '2.0',
				id: null,
				error: { code: -32700, message: 'Invalid JSON' },
			},
			{
				jsonrpc: '2.0',
				id: 2,
				error: { code: -32601, message: 'Unknown method: nope' },
			},
		]);
	});

	it('refuses malformed frames in the words the other two servers use, and a null id is a notification', async () => {
		const { replies } = await session([
			'{"id":1,"method":"ping"}\n',
			'{"jsonrpc":"1.0","id":2,"method":"ping"}\n',
			'[{"jsonrpc":"2.0","id":3,"method":"ping"}]\n',
			'{"jsonrpc":"2.0","id":null,"method":"ping"}\n',
			'{"jsonrpc":"2.0","id":4,"method":"notifications/initialized"}\n',
			'{"jsonrpc":"2.0","id":5,"method":"tools/call","params":5}\n',
			'{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"nope"}}\n',
			'{"jsonrpc":"2.0","id":7,"method":"tools/call"}\n',
			'{"jsonrpc":"2.0","id":8,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}\n',
		]);
		const refused = {
			jsonrpc: '2.0',
			id: null,
			error: { code: -32700, message: 'Not a JSON-RPC 2.0 request' },
		};
		expect(replies).toEqual([
			refused,
			refused,
			refused,
			{
				jsonrpc: '2.0',
				id: 4,
				error: {
					code: -32601,
					message: 'Unknown method: notifications/initialized',
				},
			},
			{
				jsonrpc: '2.0',
				id: 5,
				error: { code: -32602, message: 'tools/call requires a string `name`' },
			},
			{
				jsonrpc: '2.0',
				id: 6,
				error: { code: -32602, message: 'Unknown tool: nope' },
			},
			{
				jsonrpc: '2.0',
				id: 7,
				error: { code: -32602, message: 'tools/call requires a string `name`' },
			},
			{
				jsonrpc: '2.0',
				id: 8,
				result: {
					capabilities: { tools: {} },
					protocolVersion: '2025-06-18',
					serverInfo: { name: 'i18n-le', version: '9.9.9' },
				},
			},
		]);
	});

	it('waits for the pipe to drain before reading the next frame', async () => {
		const lines: string[] = [];
		let drained = 0;
		const input = Readable.from([
			Buffer.from(
				'{"jsonrpc":"2.0","id":1,"method":"ping"}\n{"jsonrpc":"2.0","id":2,"method":"ping"}\n',
			),
		]);
		const write = (line: string) => {
			lines.push(line);
			return new Promise<void>((resolve) =>
				setTimeout(() => {
					drained += 1;
					resolve();
				}, 1),
			);
		};
		await serve(input, write, '9.9.9');
		expect(lines).toHaveLength(2);
		expect(drained).toBe(2);
	});
});
