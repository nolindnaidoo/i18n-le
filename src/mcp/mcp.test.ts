import { EventEmitter } from 'node:events';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { libraryNames } from '../audit/library';
import { capped, isOk, readMaxResults } from './envelope';
import { TOOLS } from './tools';
import { createResponder, serve } from './transport';

/**
 * The MCP layer: the envelope, the one tool, and the protocol.
 *
 * The tool is shared with the crate's server, so its answers are pinned by the
 * crate's own corpus here as well as there. What this file adds is the rule
 * the tool exists under: no translated value ever leaves.
 */

const CORPUS = join(__dirname, '..', '..', 'crate', 'fixtures');
const tool = TOOLS[0] as (typeof TOOLS)[number];
// Async, so a refusal thrown before the first await arrives as a rejection,
// which is how the transport sees it.
const call = async (args: Record<string, unknown>) =>
	(await tool.handler(args)) as Record<string, any>;
const document = (name: string) =>
	readFileSync(join(CORPUS, 'documents', name), 'utf8');

describe('envelope', () => {
	it('is ok with no diagnostics and with a warning, and not with an error', () => {
		expect(isOk([])).toBe(true);
		expect(
			isOk([{ severity: 'warning', code: 'unparsable', message: 'm' }]),
		).toBe(true);
		expect(isOk([{ severity: 'error', code: 'x', message: 'm' }])).toBe(false);
	});

	it('reports truncation honestly when it drops items', () => {
		expect(capped([1, 2, 3], 2)).toEqual({ items: [1, 2], truncated: true });
		expect(capped([1, 2], 5)).toEqual({ items: [1, 2], truncated: false });
	});

	it('rejects a maxResults a tool cannot honour, and clamps an oversized one', () => {
		expect(() => readMaxResults({ maxResults: 0 })).toThrow(/positive integer/);
		expect(() => readMaxResults({ maxResults: 1.5 })).toThrow();
		expect(readMaxResults({ maxResults: 999999 })).toBe(5000);
	});
});

describe('tool table', () => {
	it('pins the tool name and offers exactly the libraries the engine reads', () => {
		expect(TOOLS.map((t) => t.name)).toEqual(['check_catalogues']);
		const properties = tool.inputSchema.properties as Record<
			string,
			{ enum?: string[] }
		>;
		expect(properties.library?.enum).toEqual(libraryNames());
	});

	it('offers no way to ask for a translated value, because nothing here returns one', () => {
		const properties = Object.keys(tool.inputSchema.properties as object);
		expect(properties).toEqual([
			'library',
			'files',
			'source',
			'keysAreSource',
			'maxResults',
		]);
	});
});

interface CorpusCase {
	readonly name: string;
	readonly files?: readonly string[];
	readonly arguments: Record<string, unknown>;
	readonly expected?: Record<string, unknown>;
	readonly expectedError?: string;
}

describe('check_catalogues: the shared corpus', () => {
	const cases = JSON.parse(
		readFileSync(join(CORPUS, 'mcp-check-catalogues.json'), 'utf8'),
	) as CorpusCase[];

	for (const testCase of cases) {
		it(testCase.name, async () => {
			const args = { ...testCase.arguments };
			if (testCase.files)
				args.files = testCase.files.map((path) => ({
					path,
					content: document(path),
				}));
			if (testCase.expectedError !== undefined) {
				await expect(call(args)).rejects.toThrow(testCase.expectedError);
				return;
			}
			const answer = JSON.parse(JSON.stringify(await call(args)));
			for (const [key, value] of Object.entries(testCase.expected ?? {}))
				expect(answer[key]).toEqual(value);
		});
	}
});

describe('check_catalogues: arguments', () => {
	const files = [{ path: 'en.json', content: '{"a":"x"}' }];

	it("refuses each malformed argument by name, in the crate's order", async () => {
		await expect(call({ files })).rejects.toThrow(/library is required/);
		await expect(call({ library: 'lingui', files })).rejects.toThrow(
			/lingui is not a library this reads/,
		);
		await expect(
			call({ library: 'i18next', files: 'en.json' }),
		).rejects.toThrow(/files is required/);
		await expect(
			call({ library: 'i18next', files: [{ path: 'en.json' }] }),
		).rejects.toThrow(/files is required/);
		await expect(
			call({ library: 'i18next', files, maxResults: -1 }),
		).rejects.toThrow(/positive integer/);
		// The library is checked before the files, so a call with both wrong is
		// refused for the same one by either server.
		await expect(call({ library: 'lingui', files: [] })).rejects.toThrow(
			/lingui/,
		);
	});

	it("reports a catalogue that will not parse with serde_json's own words, and audits the rest", async () => {
		const answer = await call({
			library: 'i18next',
			files: [
				{ path: 'en.json', content: '{"a": "Hello {{name}}"}' },
				{ path: 'de.json', content: '{\n  "a": "Hallo" "b"\n}' },
			],
		});
		expect(answer.diagnostics).toEqual([
			{
				severity: 'warning',
				code: 'unparsable',
				message: 'de.json: expected `,` or `}` at line 2 column 16',
			},
		]);
		expect(answer.data.files).toHaveLength(1);
		expect(answer.ok).toBe(true);
	});
});

describe('the privacy boundary', () => {
	it('never returns a translated string, only keys, tokens and counts', async () => {
		const answer = await call({
			library: 'i18next',
			files: [
				{
					path: 'en.json',
					content:
						'{"greeting": "Hello {{name}}", "bye": "Goodbye", "same": "OK"}',
				},
				{
					path: 'es.json',
					content:
						'{"greeting": "Hola {{nombre}}", "bye": "  ", "same": "OK", "extra": "Sobrante"}',
				},
			],
		});
		const text = JSON.stringify(answer);
		for (const translated of ['Hola', 'Sobrante', 'Hello', 'Goodbye'])
			expect(text).not.toContain(translated);
		expect(answer.data.findings.map((f: { kind: string }) => f.kind)).toEqual([
			'extra-key',
			'placeholder-name-mismatch',
			'untranslated',
			'empty-value',
		]);
	});
});

describe('protocol', () => {
	const respond = createResponder({ name: 'i18n-le', version: '1.0.0' }, TOOLS);

	it('echoes the protocol version the client asked for', async () => {
		const reply = await respond({
			jsonrpc: '2.0',
			id: 1,
			method: 'initialize',
			params: { protocolVersion: '2024-11-05' },
		});
		expect(reply?.result?.protocolVersion).toBe('2024-11-05');
		expect(reply?.result?.serverInfo).toEqual({
			name: 'i18n-le',
			version: '1.0.0',
		});
	});

	it('does not reply to a notification', async () => {
		// A reply to a notification is the classic way to wedge a client.
		expect(
			await respond({ jsonrpc: '2.0', method: 'notifications/initialized' }),
		).toBeNull();
	});

	it('reports an unknown method as a JSON-RPC error', async () => {
		const reply = await respond({ jsonrpc: '2.0', id: 2, method: 'nope' });
		expect(reply?.error?.code).toBe(-32601);
	});

	it('reports an unknown tool without killing the connection', async () => {
		const reply = await respond({
			jsonrpc: '2.0',
			id: 3,
			method: 'tools/call',
			params: { name: 'no_such_tool', arguments: {} },
		});
		expect(reply?.error?.code).toBe(-32602);
	});

	it('returns a tool failure as a result, not a protocol error', async () => {
		// A model can read an isError result and correct itself; a JSON-RPC error
		// reads as "the server is broken".
		const reply = await respond({
			jsonrpc: '2.0',
			id: 4,
			method: 'tools/call',
			params: { name: 'check_catalogues', arguments: {} },
		});
		expect(reply?.error).toBeUndefined();
		expect(reply?.result?.isError).toBe(true);
	});
});

describe('serve: the stdio loop', () => {
	/** A fake stdin/stdout pair so the loop can be driven without a process. */
	function harness() {
		const input = new EventEmitter() as EventEmitter & {
			setEncoding?: (e: string) => void;
		};
		const written: string[] = [];
		const output = {
			write: (chunk: string) => {
				written.push(chunk);
				return true;
			},
		};
		serve(
			{ name: 'i18n-le', version: '1.0.0' },
			TOOLS,
			input as never,
			output as never,
		);
		const replies = () =>
			written
				.join('')
				.split('\n')
				.filter(Boolean)
				.map((l) => JSON.parse(l));
		return { input, replies };
	}

	const settle = () => new Promise((r) => setTimeout(r, 20));

	it('answers a request delivered as one line', async () => {
		const { input, replies } = harness();
		input.emit('data', '{"jsonrpc":"2.0","id":1,"method":"tools/list"}\n');
		await settle();
		expect(replies()[0]?.result?.tools).toHaveLength(1);
	});

	it('reassembles a request split across chunks', async () => {
		// stdin delivers whatever the OS gives it; a request arriving in two
		// pieces must not be dropped or double-parsed.
		const { input, replies } = harness();
		input.emit('data', '{"jsonrpc":"2.0","id":2,"me');
		input.emit('data', 'thod":"ping"}\n');
		await settle();
		expect(replies()[0]?.id).toBe(2);
	});

	it('handles several requests in one chunk', async () => {
		const { input, replies } = harness();
		input.emit(
			'data',
			'{"jsonrpc":"2.0","id":3,"method":"ping"}\n{"jsonrpc":"2.0","id":4,"method":"ping"}\n',
		);
		await settle();
		expect(replies().map((r) => r.id)).toEqual([3, 4]);
	});

	it('reports malformed JSON without dying', async () => {
		// One bad line from a client must not take the server down for everyone.
		const { input, replies } = harness();
		input.emit('data', 'not json at all\n');
		input.emit('data', '{"jsonrpc":"2.0","id":5,"method":"ping"}\n');
		await settle();
		expect(replies()[0]?.error?.code).toBe(-32700);
		expect(replies()[1]?.id).toBe(5);
	});

	it('rejects a payload that is not a JSON-RPC request', async () => {
		const { input, replies } = harness();
		input.emit('data', '{"hello":"world"}\n');
		await settle();
		expect(replies()[0]?.error?.code).toBe(-32700);
	});

	it('ignores blank lines', async () => {
		const { input, replies } = harness();
		input.emit('data', '\n\n{"jsonrpc":"2.0","id":6,"method":"ping"}\n');
		await settle();
		expect(replies()).toHaveLength(1);
	});

	it('writes nothing for a notification', async () => {
		const { input, replies } = harness();
		input.emit(
			'data',
			'{"jsonrpc":"2.0","method":"notifications/initialized"}\n',
		);
		await settle();
		expect(replies()).toHaveLength(0);
	});
});
