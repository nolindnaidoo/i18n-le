import { describe, expect, it } from 'vitest';
import { reportFor } from '../audit/report';
import { describeEvidence, formatReport } from './format';

const audit = (
	files: Array<[string, string | undefined, string]>,
	library: 'i18next' | 'vscode-l10n' = 'i18next',
) =>
	reportFor(
		files.map(([name, locale, content]) => ({ name, locale, content })),
		{
			library,
			version: '^26.2.0',
			layout: { shape: 'shared', extension: 'json' },
			keysAreSource: false,
			evidence: [],
		},
		{ source: undefined, keysAreSource: false },
	);

describe('the report', () => {
	it('opens with the library, its version, the layout and the count', () => {
		const text = formatReport(
			audit([
				['en.json', 'en', '{"a":"x"}'],
				['de.json', 'de', '{}'],
			]),
			true,
		);
		expect(text).toContain(
			'**i18next** · ^26.2.0 · `a directory of <locale>.json` · 2 catalogue(s) · 1 finding(s)',
		);
		expect(text).toContain('- Named in the i18n-le.library setting.');
		expect(text).toContain('- `en.json` · en · 1 key(s) · source');
	});

	it('says a clean set is clean, and an empty directory has nothing in it', () => {
		expect(
			formatReport(
				audit([
					['en.json', 'en', '{"a":"x"}'],
					['de.json', 'de', '{"a":"y"}'],
				]),
				false,
			),
		).toContain('nothing structurally wrong');
		expect(formatReport(audit([]), false)).toContain(
			'no catalogues found here',
		);
	});

	it('words every kind of evidence without quoting a translation', () => {
		const text = formatReport(
			audit([
				[
					'en.json',
					'en',
					'{"a":"Hi {{n}}","b":"Hi {{n}}","c":{"d":"x"},"e":"Hi {{n}}","f":"Hey"}',
				],
				[
					'es.json',
					'es',
					'{"a":"Hola {{m}}","b":"Hola","c":"plano","e":"Hola {n}","f":"Hola %s","f":"Hola %s"}',
				],
			]),
			false,
		);
		expect(text).toContain('source `n`, target `m`');
		expect(text).toContain('1 placeholder(s) in the source, 0 here');
		expect(text).toContain('source object, target text');
		expect(text).toContain('source double-brace, target single-brace');
		expect(text).toContain('printf at byte 5');
		expect(text).toContain('defined 2 times');
		for (const translated of ['Hola', 'plano'])
			expect(text).not.toContain(translated);
	});

	it('keeps a backtick in a key from closing its code span, and shows an empty key as ""', () => {
		const text = formatReport(
			audit([
				['en.json', 'en', '{"a`b":"x","":"y"}'],
				['de.json', 'de', '{}'],
			]),
			false,
		);
		expect(text).toContain("**`a'b`** · missing-key");
		expect(text).toContain('**`""`** · missing-key');
	});

	it('lists what it could not read, with the reason', () => {
		const text = formatReport(
			audit([
				['en.json', 'en', '{"a":"x"}'],
				['de.json', 'de', '{ broken'],
			]),
			false,
		);
		expect(text).toContain(
			'- `de.json` (unparsable): key must be a string at line 1 column 3',
		);
	});

	it('has nothing to say about a finding with no evidence', () => {
		expect(
			describeEvidence({
				severity: 'error',
				kind: 'missing-key',
				file: 'x',
				key: 'k',
			}),
		).toBe('');
	});
});
