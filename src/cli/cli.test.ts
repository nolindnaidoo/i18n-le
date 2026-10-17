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
		expect(err.join('')).toBe(
			'i18n-le: no-such-dir-here: No such file or directory (os error 2)\n',
		);
	});
});

describe('mcp', () => {
	it('offers the one shared tool, keys sorted', async () => {
		const written: string[] = [];
		const request = { jsonrpc: '2.0', id: 1, method: 'tools/list' };
		await serve(
			Readable.from([Buffer.from(`${JSON.stringify(request)}\n`)]),
			(reply) => written.push(reply),
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
