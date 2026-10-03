import { describe, expect, it } from 'vitest';
import type { EntryType, FileSystem } from './fs';
import { identify } from './identify';
import { scan } from './scan';

/** A tree in memory: absolute path to text. A directory is any path a file sits under. */
function memory(files: Record<string, string>): FileSystem {
	const has = (path: string) => Object.hasOwn(files, path);
	const under = (path: string) =>
		Object.keys(files).filter((file) =>
			file.startsWith(path === '/' ? '/' : `${path}/`),
		);
	return {
		async readDirectory(path) {
			const entries = new Map<string, EntryType>();
			for (const file of under(path)) {
				const [name, ...rest] = file
					.slice(path === '/' ? 1 : path.length + 1)
					.split('/');
				entries.set(name as string, rest.length === 0 ? 'file' : 'directory');
			}
			return [...entries];
		},
		async stat(path) {
			if (has(path))
				return { type: 'file', size: (files[path] as string).length };
			return under(path).length > 0
				? { type: 'directory', size: 0 }
				: undefined;
		},
		async readFile(path) {
			if (!has(path)) throw new Error('No such file or directory (os error 2)');
			return new TextEncoder().encode(files[path]);
		},
	};
}

const HOW = 'with --system <name>';
const run = (
	files: Record<string, string>,
	input: string,
	named?: 'i18next' | 'next-intl',
) => identify({ fs: memory(files), inputs: [input], named, howToName: HOW });

describe('identification', () => {
	it('identifies from two agreeing classes and reports the evidence for the winner only', async () => {
		const found = await run(
			{
				'/p/package.json':
					'{"dependencies":{"react-i18next":"^15.0.0","i18next":"^26.2.0"}}',
				'/p/locales/en.json': '{"a":"Hello {{name}}"}',
				'/p/locales/de.json': '{"a":"Hallo {{name}}"}',
			},
			'/p/locales',
		);
		expect(found.library).toBe('i18next');
		// serde_json's map is sorted, so the manifest is read in key order.
		expect(found.evidence.map((s) => `${s.class}: ${s.detail}`)).toEqual([
			'manifest: i18next in ../package.json',
			'manifest: react-i18next in ../package.json',
			'layout: a directory of <locale>.json',
			'content: double-brace',
		]);
		expect(found.version).toBe('^26.2.0');
	});

	it('identifies an ARB directory from its syntax alone', async () => {
		const found = await run(
			{
				'/f/l10n/app_en.arb': '{"@@locale":"en","hi":"Hi","@hi":{}}',
				'/f/l10n/app_es.arb': '{"@@locale":"es","hi":"Hola","@hi":{}}',
			},
			'/f/l10n',
		);
		expect(found.library).toBe('flutter-arb');
		expect(found.files.map((f) => [f.name, f.locale])).toEqual([
			['app_en.arb', 'en'],
			['app_es.arb', 'es'],
		]);
	});

	it('refuses two identified libraries, naming the evidence for each', async () => {
		await expect(
			run(
				{
					'/p/package.json':
						'{"dependencies":{"i18next":"^26.0.0","next-intl":"^4.0.0"}}',
					'/p/src/a.ts': 'useTranslation(); useTranslations();',
					'/p/x/en.json': '{}',
				},
				'/p/x',
			),
		).rejects.toThrow(
			'2 libraries are identified here and only one can be right: i18next (manifest: i18next in ../package.json, call site: useTranslation( in ../src/a.ts); next-intl (manifest: next-intl in ../package.json, call site: useTranslations( in ../src/a.ts). Name the one these catalogues belong to with --system <name>.',
		);
	});

	it('refuses when nothing agrees, listing what it did find', async () => {
		await expect(
			run(
				{
					'/p/package.json': '{"dependencies":{"next-intl":"4"}}',
					'/p/x/en.json': '{}',
				},
				'/p/x',
			),
		).rejects.toThrow(
			'no i18n library could be identified here. Found, but not enough to agree: next-intl manifest: next-intl in ../package.json. Two classes of evidence are needed. Name it with --system <name>.',
		);
	});

	it('refuses an i18next major that wrote plurals differently', async () => {
		await expect(
			run(
				{
					'/p/package.json': '{"dependencies":{"i18next":"^22.4.0"}}',
					'/p/locales/en.json': '{"a":"{{x}}"}',
				},
				'/p/locales',
			),
		).rejects.toThrow(
			/i18next \^22\.4\.0 in \.\.\/package\.json writes its catalogues differently from 23/,
		);
	});

	it('reads a VS Code manifest bundle split across two directories', async () => {
		const found = await run(
			{
				'/e/package.json': '{"l10n":"./l10n","dependencies":{}}',
				'/e/package.nls.json': '{"a":"Open"}',
				'/e/src/i18n/package.nls.de.json': '{"a":"Öffnen"}',
				'/e/src/i18n/package.nls.zh-cn.json': '{"a":"打开"}',
				'/e/src/extension.ts': 'vscode.l10n.t("x")',
			},
			'/e/src/i18n',
		);
		expect(found.library).toBe('vscode-l10n');
		expect(found.files.map((f) => [f.name, f.locale ?? null])).toEqual([
			['package.nls.json', null],
			['package.nls.de.json', 'de'],
			['package.nls.zh-cn.json', 'zh-CN'],
		]);
	});

	it('refuses a namespaced set holding two namespaces', async () => {
		await expect(
			run(
				{
					'/p/locales/en/common.json': '{}',
					'/p/locales/en/errors.json': '{}',
					'/p/locales/de/common.json': '{}',
				},
				'/p/locales',
				'i18next',
			),
		).rejects.toThrow(
			'this set has 2 namespaces (common.json, errors.json) and a namespace is its own set.',
		);
	});

	it('takes a Flutter version from pubspec.yaml, past a package declared without one', async () => {
		const found = await run(
			{
				'/f/pubspec.yaml':
					'dependencies:\r\n  flutter_localizations:\r\n    sdk: flutter\r\n  intl: ^0.19.0\r\n',
				'/f/lib/l10n/app_en.arb': '{"@@locale":"en","a":"x","@a":{}}',
			},
			'/f/lib/l10n',
		);
		expect(found.version).toBe('^0.19.0');
	});

	it('counts a config file only in an extension that library writes', async () => {
		const found = await run(
			{
				'/p/l10n.json': '{}',
				'/p/next-intl.config.ts': 'export default {}',
				'/p/package.json': '{"dependencies":{"next-intl":"^4"}}',
				'/p/messages/en.json': '{"a":"{n}"}',
			},
			'/p/messages',
		);
		expect(found.library).toBe('next-intl');
		expect(found.evidence.some((s) => s.detail === 'l10n.json')).toBe(false);
		expect(
			found.evidence.some(
				(s) => s.class === 'config' && s.detail === '../next-intl.config.ts',
			),
		).toBe(true);
	});

	it("ignores a manifest that will not parse, rather than refusing over somebody else's file", async () => {
		await expect(
			run(
				{
					'/p/package.json': '{ broken',
					'/p/l10n/app_en.arb': '{"@@locale":"en","a":"x","@a":{}}',
				},
				'/p/l10n',
			),
		).resolves.toMatchObject({ library: 'flutter-arb' });
	});

	it('reads an npm alias major and leaves an unparsable range alone', async () => {
		await expect(
			run(
				{
					'/p/package.json':
						'{"dependencies":{"i18next":"npm:i18next-fork@21.0.0"}}',
					'/p/locales/en.json': '{"a":"{{x}}"}',
				},
				'/p/locales',
			),
		).rejects.toThrow(/writes its catalogues differently from 23/);
		await expect(
			run(
				{
					'/p/package.json': '{"dependencies":{"i18next":"latest"}}',
					'/p/locales/en.json': '{"a":"{{x}}"}',
				},
				'/p/locales',
			),
		).resolves.toMatchObject({ library: 'i18next', version: 'latest' });
	});
});

describe('the scan', () => {
	it('names a catalogue it could not read and audits the rest', async () => {
		const fs = memory({ '/x/en.json': '{"a":"b"}', '/x/de.json': '{"a":"c"}' });
		const broken: FileSystem = {
			...fs,
			readFile: async (path) =>
				path.endsWith('de.json')
					? Promise.reject(new Error('Permission denied (os error 13)'))
					: fs.readFile(path),
		};
		const report = await scan({
			fs: broken,
			inputs: ['/x'],
			named: 'i18next',
			source: undefined,
			keysAreSource: false,
			howToName: HOW,
		});
		expect(report.diagnostics).toEqual([
			{
				severity: 'warning',
				code: 'skipped',
				file: 'de.json',
				message: 'Permission denied (os error 13)',
			},
		]);
		expect(report.files.map((f) => f.path)).toEqual(['en.json']);
	});

	it('reports a directory with no catalogues as no-files, which is not a failure', async () => {
		const report = await scan({
			fs: memory({ '/x/readme.md': '#' }),
			inputs: ['/x'],
			named: 'i18next',
			source: undefined,
			keysAreSource: false,
			howToName: HOW,
		});
		expect(report.status).toBe('no-files');
	});
});
