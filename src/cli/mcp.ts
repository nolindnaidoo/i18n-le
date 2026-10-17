/**
 * `i18n-le mcp`: the crate's MCP server (`crate/src/mcp/`), not the
 * extension's. It offers the shared `check_catalogues`, answers in the
 * crate's words, and writes every reply with its keys sorted, because the
 * crate builds each one as a `serde_json::Value` without `preserve_order`.
 */

import { sortedKeys } from '../mcp/envelope';
import { TOOLS } from '../mcp/tools';

const PROTOCOL_VERSION = '2025-06-18';
const INVALID_PARAMS = -32602;
const METHOD_NOT_FOUND = -32601;

type Handled =
	| { readonly result: unknown }
	| { readonly error: { readonly code: number; readonly message: string } };

const SHARED = sharedTool('check_catalogues');

/** The shared tool is the extension's, definition and handler both. */
function sharedTool(name: string) {
	const tool = TOOLS.find((candidate) => candidate.name === name);
	if (!tool) throw new Error(`the extension offers no tool named ${name}`);
	return tool;
}

/**
 * Serves until stdin closes, one request at a time, in order. A line that is
 * not UTF-8 ends the server with exit 2, as `BufRead::lines` does.
 */
export async function serve(
	input: AsyncIterable<Uint8Array>,
	write: (line: string) => void,
	version: string,
): Promise<number> {
	for await (const line of lines(input)) {
		if (line === null) return 2;
		if (isRustBlank(line)) continue;
		let request: unknown;
		try {
			request = JSON.parse(line);
		} catch {
			// A frame that is not JSON has no id to answer against.
			continue;
		}
		const response = await handle(request, version);
		if (response !== undefined) write(`${response}\n`);
	}
	return 0;
}

/** Lines split on `\n` with one trailing `\r` dropped, or null for invalid UTF-8. */
async function* lines(
	input: AsyncIterable<Uint8Array>,
): AsyncGenerator<string | null> {
	const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
	let pending = Buffer.alloc(0);
	for await (const chunk of input) {
		pending = Buffer.concat([pending, chunk]);
		let newline = pending.indexOf(10);
		while (newline !== -1) {
			yield decodeLine(decoder, pending.subarray(0, newline));
			pending = pending.subarray(newline + 1);
			newline = pending.indexOf(10);
		}
	}
	if (pending.length > 0) yield decodeLine(decoder, pending);
}

function decodeLine(decoder: TextDecoder, bytes: Uint8Array): string | null {
	try {
		const line = decoder.decode(bytes);
		return line.endsWith('\r') ? line.slice(0, -1) : line;
	} catch {
		return null;
	}
}

/** Rust's `char::is_whitespace`, which `str::trim` strips. */
function isRustBlank(line: string): boolean {
	return /^[\t\n\v\f\r \u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]*$/.test(
		line,
	);
}

function field(value: unknown, name: string): unknown {
	if (typeof value !== 'object' || value === null || Array.isArray(value)) {
		return undefined;
	}
	return Object.hasOwn(value, name)
		? (value as Record<string, unknown>)[name]
		: undefined;
}

async function handle(
	request: unknown,
	version: string,
): Promise<string | undefined> {
	const id = field(request, 'id');
	const method = field(request, 'method');
	if (typeof method !== 'string') return undefined;
	// A notification carries no id and gets no reply.
	if (id === undefined) return undefined;

	const handled = await dispatch(method, field(request, 'params'), version);
	return JSON.stringify(sortedKeys({ jsonrpc: '2.0', id, ...handled }));
}

async function dispatch(
	method: string,
	params: unknown,
	version: string,
): Promise<Handled> {
	if (method === 'initialize') {
		return {
			result: {
				protocolVersion: PROTOCOL_VERSION,
				capabilities: { tools: {} },
				serverInfo: { name: 'i18n-le', version },
			},
		};
	}
	if (method === 'tools/list') {
		return {
			result: {
				tools: [
					{
						name: SHARED.name,
						description: SHARED.description,
						inputSchema: SHARED.inputSchema,
					},
				],
			},
		};
	}
	if (method === 'tools/call') return callTool(params);
	if (method === 'ping') return { result: {} };
	return {
		error: {
			code: METHOD_NOT_FOUND,
			message: `this server does not implement ${method}`,
		},
	};
}

/**
 * Protocol failures are JSON-RPC errors. A tool that fails on its arguments
 * returns a result carrying `isError`, so a model reads the reason rather than
 * concluding the server is broken.
 */
async function callTool(params: unknown): Promise<Handled> {
	if (params === undefined) {
		return {
			error: { code: INVALID_PARAMS, message: 'no tool call was supplied' },
		};
	}
	const name = field(params, 'name');
	if (typeof name !== 'string') {
		return {
			error: { code: INVALID_PARAMS, message: 'the tool call named no tool' },
		};
	}
	// serde_json answers `get` on anything but an object with nothing, so
	// arguments that are not an object read as an empty one.
	const raw = field(params, 'arguments');
	const args = isObject(raw) ? raw : {};
	if (name === 'check_catalogues') {
		return { result: await outcome(() => SHARED.handler(args)) };
	}
	return {
		error: {
			code: INVALID_PARAMS,
			message: `this server offers no tool named ${name}`,
		},
	};
}

function isObject(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

async function outcome(tool: () => Promise<unknown>): Promise<unknown> {
	let envelope: unknown;
	try {
		envelope = sortedKeys(await tool());
	} catch (error) {
		const message = error instanceof Error ? error.message : String(error);
		return { content: [{ type: 'text', text: message }], isError: true };
	}
	return {
		content: [{ type: 'text', text: JSON.stringify(envelope, null, 2) }],
		structuredContent: envelope,
		isError: false,
	};
}
