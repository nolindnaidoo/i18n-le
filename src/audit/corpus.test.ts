import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { parseCatalogue } from './catalogue';
import type { LibraryId, Mark } from './library';
import { localesOf } from './locale';
import { keyMarks, marks } from './message';
import { reportFor } from './report';

/**
 * The crate's `fixtures/detection.json`, run through the port: the same four
 * checks `crate/src/corpus.rs` runs, so a difference is a bug in one of them.
 */

const FIXTURES = join(__dirname, '..', '..', 'crate', 'fixtures');
const document = (name: string) =>
	readFileSync(join(FIXTURES, 'documents', name), 'utf8');

interface Corpus {
	parsing: Array<{
		file: string;
		keys?: string[];
		duplicates?: Array<{ key: string; occurrences: number }>;
		refused?: boolean;
	}>;
	locales: Array<{
		names: string[];
		locales?: Array<string | null>;
		refused?: boolean;
	}>;
	marks: Array<{ file: string; marks: Mark[] }>;
	audit: Array<{
		name: string;
		library: LibraryId;
		files: string[];
		options?: { source?: string; keysAreSource?: boolean };
		expected: unknown;
	}>;
}

const corpus = JSON.parse(
	readFileSync(join(FIXTURES, 'detection.json'), 'utf8'),
) as Corpus;

describe('the shared corpus: parsing', () => {
	for (const testCase of corpus.parsing) {
		it(testCase.file, () => {
			if (testCase.refused) {
				expect(() => parseCatalogue(document(testCase.file))).toThrow();
				return;
			}
			const parsed = parseCatalogue(document(testCase.file));
			expect(parsed.entries.map((entry) => entry.key)).toEqual(testCase.keys);
			expect(parsed.duplicates).toEqual(testCase.duplicates ?? []);
		});
	}
});

describe('the shared corpus: locales', () => {
	for (const testCase of corpus.locales) {
		it(testCase.names.join(', '), () => {
			if (testCase.refused) {
				expect(() => localesOf(testCase.names)).toThrow(
					/is not a language tag/,
				);
				return;
			}
			expect(localesOf(testCase.names).map((locale) => locale ?? null)).toEqual(
				testCase.locales,
			);
		});
	}
});

describe('the shared corpus: marks', () => {
	for (const testCase of corpus.marks) {
		it(testCase.file, () => {
			const parsed = parseCatalogue(document(testCase.file));
			const found = keyMarks(parsed.entries.map((entry) => entry.key));
			for (const entry of parsed.entries) {
				for (const mark of entry.text === undefined ? [] : marks(entry.text)) {
					if (!found.includes(mark)) found.push(mark);
				}
			}
			expect([...found].sort()).toEqual([...testCase.marks].sort());
		});
	}
});

describe('the shared corpus: audits', () => {
	for (const testCase of corpus.audit) {
		it(testCase.name, () => {
			const keysAreSource = testCase.options?.keysAreSource ?? false;
			const locales = localesOf(testCase.files);
			const report = reportFor(
				testCase.files.map((name, index) => ({
					name,
					locale: locales[index],
					content: document(name),
				})),
				{
					library: testCase.library,
					version: null,
					layout: null,
					keysAreSource,
					evidence: [],
				},
				{ source: testCase.options?.source, keysAreSource },
			);
			expect(JSON.parse(JSON.stringify(report))).toEqual(testCase.expected);
		});
	}
});
