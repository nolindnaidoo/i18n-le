import { describe, expect, it } from 'vitest';
import { parseCatalogue } from './catalogue';
import { parseJson } from './json';

/**
 * serde_json's refusals, word for word and byte column for byte column. Each
 * expectation here was read back from the crate's own server for the same
 * input, not written from memory; the differential checks thousands more.
 */
const refusal = (text: string) => {
	try {
		parseJson(text);
	} catch (error) {
		return (error as Error).message;
	}
	return 'parsed';
};

describe('serde_json refusals', () => {
	it.each([
		['', 'EOF while parsing a value at line 1 column 0'],
		['x', 'expected value at line 1 column 1'],
		['{"a" 1}', 'expected `:` at line 1 column 6'],
		['{"a": 1,}', 'trailing comma at line 1 column 9'],
		['{"a": 1 "b": 2}', 'expected `,` or `}` at line 1 column 9'],
		['{1: 2}', 'key must be a string at line 1 column 2'],
		['{"a": tru}', 'expected ident at line 1 column 10'],
		['{"a": 01}', 'invalid number at line 1 column 8'],
		['{"a": 1.}', 'invalid number at line 1 column 9'],
		['{"a": 1e400}', 'number out of range at line 1 column 11'],
		['{"a": "\\ud800"}', 'unexpected end of hex escape at line 1 column 14'],
		[
			'{"a": "\\udc00"}',
			'lone leading surrogate in hex escape at line 1 column 13',
		],
		['{"a": "\\x"}', 'invalid escape at line 1 column 9'],
		[
			'{"a": "tab\there"}',
			'control character (\\u0000-\\u001F) found while parsing a string at line 1 column 11',
		],
		['{"a": "open', 'EOF while parsing a string at line 1 column 11'],
		['{} x', 'trailing characters at line 1 column 4'],
		['{"é": 1 2}', 'expected `,` or `}` at line 1 column 10'],
		[`${'{"a":'.repeat(127)}1${'}'.repeat(127)}`, 'parsed'],
		[
			`${'{"a":'.repeat(128)}1${'}'.repeat(128)}`,
			'recursion limit exceeded at line 1 column 636',
		],
	])('%j', (text, expected) => {
		expect(refusal(text)).toBe(expected);
	});

	// Numbers, array draining and escapes, each read back from the crate's own
	// server: the paths where serde_json is least like JSON.parse.
	it.each([
		['{"v": 1.5e-400}', 'parsed'],
		['{"v": -0}', 'parsed'],
		['{"v": 18446744073709551615}', 'parsed'],
		['{"v": 18446744073709551616}', 'parsed'],
		['{"v": 123456789012345678901234567890}', 'parsed'],
		['{"v": 1.7976931348623157e308}', 'parsed'],
		['{"v": 1.8e308}', 'number out of range at line 1 column 13'],
		[
			'{"v": 123456789012345678901234567890e300}',
			'number out of range at line 1 column 40',
		],
		['{"v": 0.00000000000000000000000000000000000000001e-300}', 'parsed'],
		['{"v": 1e99999999999}', 'number out of range at line 1 column 18'],
		['{"v": 1e-99999999999}', 'parsed'],
		['{"v": 0e99999999999}', 'parsed'],
		['{"v": 12345678901234567890.123456789012345678901234567890}', 'parsed'],
		['{"v": 1.}', 'invalid number at line 1 column 9'],
		['{"v": 1e}', 'invalid number at line 1 column 9'],
		['{"v": 1e+}', 'invalid number at line 1 column 10'],
		['{"v": -}', 'invalid number at line 1 column 8'],
		['{"v": -a}', 'invalid number at line 1 column 8'],
		['{"v": 01}', 'invalid number at line 1 column 8'],
		['{"v": 1.5E+2}', 'parsed'],
		['{"v": [1, 2]}', 'parsed'],
		['{"v": [1 2]}', 'expected `,` or `]` at line 1 column 10'],
		['{"v": [1,]}', 'trailing comma at line 1 column 10'],
		['{"v": [{"a" 1}]}', 'expected `:` at line 1 column 13'],
		['{"v": [{"a": 1,}]}', 'key must be a string at line 1 column 16'],
		['{"v": [{1: 2}]}', 'key must be a string at line 1 column 9'],
		['{"v": [tru]}', 'expected ident at line 1 column 11'],
		['{"v": [01]}', 'invalid number at line 1 column 9'],
		['{"v": [1.]}', 'invalid number at line 1 column 10'],
		['{"v": [1e]}', 'invalid number at line 1 column 10'],
		['{"v": [-]}', 'invalid number at line 1 column 9'],
		['{"v": [1.e5]}', 'invalid number at line 1 column 10'],
		['{"v": ["a\\u0041\\ud83d\\ude00"]}', 'parsed'],
		['{"v": ["\\x"]}', 'invalid escape at line 1 column 10'],
		['{"v": ["\\u12G4"]}', 'invalid escape at line 1 column 14'],
		[
			'{"v": ["tab\there"]}',
			'control character (\\u0000-\\u001F) found while parsing a string at line 1 column 11',
		],
		['{"v": [{"a": [1, {"b": null}]}]}', 'parsed'],
		['{"v": [{"a"}]}', 'expected `:` at line 1 column 12'],
		['{"v": [{"a":}', 'expected value at line 1 column 13'],
		['{"v": [1}', 'expected `,` or `]` at line 1 column 9'],
		['{"v": [{}', 'EOF while parsing a list at line 1 column 9'],
		['{"v": "\\ud83d\\ude00"}', 'parsed'],
		['{"v": "\\ud83d"}', 'unexpected end of hex escape at line 1 column 14'],
		['{"v": "\\ud83dx"}', 'unexpected end of hex escape at line 1 column 14'],
		['{"v": "\\ud83d\\x"}', 'unexpected end of hex escape at line 1 column 15'],
		[
			'{"v": "\\ud83d\\u0041"}',
			'lone leading surrogate in hex escape at line 1 column 19',
		],
		['{"v": nul}', 'expected ident at line 1 column 10'],
		['{"v": falsy}', 'expected ident at line 1 column 11'],
		['{"v": true}', 'parsed'],
		['{"v": [{"a": 1} {"b": 2}]}', 'expected `,` or `]` at line 1 column 17'],
		['{"v": [[1] 2]}', 'expected `,` or `]` at line 1 column 12'],
	])('%j', (text, expected) => {
		expect(refusal(text)).toBe(expected);
	});

	it('drains an array without a depth limit, as IgnoredAny does', () => {
		expect(refusal(`{"a": [${'['.repeat(300)}${']'.repeat(300)}]}`)).toBe(
			'parsed',
		);
	});
});

describe('catalogues', () => {
	it('keeps a string that begins with U+FEFF, which a decoder would drop as framing', () => {
		const parsed = parseCatalogue('{"a": "﻿x"}');
		expect(parsed.entries[0]?.text).toBe('﻿x');
	});

	it('strips one leading byte-order mark from the document', () => {
		expect(parseCatalogue('﻿{"a":"b"}').entries).toHaveLength(1);
	});

	it('keeps the last of a duplicated key and reports how many times it appeared', () => {
		const parsed = parseCatalogue('{"a":"1","a":"2","a":"3"}');
		expect(parsed.entries).toEqual([{ key: 'a', shape: 'text', text: '3' }]);
		expect(parsed.duplicates).toEqual([{ key: 'a', occurrences: 3 }]);
	});

	it('refuses a document that is not an object', () => {
		expect(() => parseCatalogue('[]')).toThrow(
			'a catalogue must be a JSON object',
		);
	});
});
