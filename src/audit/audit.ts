import type { Duplicate, Entry, Shape } from './catalogue';
import {
	type Interpolation,
	isMetadata,
	type Library,
	type LibraryId,
	libraryOf,
} from './library';
import { type Construct, parseMessage, pluralBase } from './message';
import { isBlank } from './text';

/**
 * The checks — the crate's `audit.rs`.
 *
 * **One named source is the reference, never the union of all of them.** A
 * union would make one locale's typo'd key a requirement every other locale
 * is then missing.
 *
 * **No translated value crosses this boundary.** A finding's evidence is
 * token names, counts, styles, shapes, occurrence counts or a construct with a
 * byte offset — none of which can hold a sentence. Keys are the exception: a
 * finding that will not name its key is not a finding.
 */

export type Severity = 'error' | 'warning' | 'info';

export type Kind =
	| 'missing-key'
	| 'extra-key'
	| 'placeholder-count-mismatch'
	| 'placeholder-name-mismatch'
	| 'placeholder-style-mismatch'
	| 'convention-mismatch'
	| 'empty-value'
	| 'untranslated'
	| 'duplicate-key-within-file'
	| 'structure-mismatch';

export type Evidence =
	| {
			readonly sourceTokens: readonly string[];
			readonly targetTokens: readonly string[];
	  }
	| { readonly sourceCount: number; readonly targetCount: number }
	| { readonly sourceStyle: Interpolation; readonly targetStyle: Interpolation }
	| { readonly sourceShape: Shape; readonly targetShape: Shape }
	| { readonly occurrences: number }
	| { readonly construct: Construct; readonly offset: number };

/** Field for field the crate's serialized finding: the evidence is flattened in. */
export type Finding = {
	readonly severity: Severity;
	readonly kind: Kind;
	readonly file: string;
	readonly key: string;
} & Partial<Record<string, unknown>>;

export interface Catalogue {
	readonly path: string;
	readonly locale: string | undefined;
	readonly entries: readonly Entry[];
	readonly duplicates: readonly Duplicate[];
}

export interface AuditOptions {
	readonly library: LibraryId;
	readonly keysAreSource: boolean;
}

/** Translatable keys: the objects on the way down are structure. */
export function keyCount(file: Catalogue): number {
	return file.entries.filter((entry) => entry.shape !== 'object').length;
}

/** Drop the keys this library calls metadata, after identification has read them. */
export function withoutMetadata(file: Catalogue, library: Library): Catalogue {
	return {
		...file,
		entries: file.entries.filter((entry) => !isMetadata(library, entry.key)),
		duplicates: file.duplicates.filter(
			(duplicate) => !isMetadata(library, duplicate.key),
		),
	};
}

function finding(
	severity: Severity,
	kind: Kind,
	file: string,
	key: string,
	evidence?: Evidence,
): Finding {
	return { severity, kind, file, key, ...evidence };
}

/** Audit every catalogue in the set against the one at `sourceIndex`. */
export function audit(
	files: readonly Catalogue[],
	sourceIndex: number,
	options: AuditOptions,
): Finding[] {
	const library = libraryOf(options.library);
	const reference = files[sourceIndex] as Catalogue;

	const findings: Finding[] = [...conventionFindings(reference, options)];
	files.forEach((file, index) => {
		if (index === sourceIndex) {
			findings.push(...duplicateFindings(file), ...emptyFindings(file));
			return;
		}
		against(reference, file, library, options, findings);
	});
	return findings;
}

/** Constructs from another convention, which the loader renders verbatim. */
function conventionFindings(file: Catalogue, options: AuditOptions): Finding[] {
	const grammar = libraryOf(options.library).grammar;
	return file.entries.flatMap((entry) => {
		const text = sourceText(entry, options.keysAreSource);
		if (text === undefined) return [];
		return parseMessage(grammar, text).foreign.map((found) =>
			finding('error', 'convention-mismatch', file.path, entry.key, {
				construct: found.construct,
				offset: found.offset,
			}),
		);
	});
}

function duplicateFindings(file: Catalogue): Finding[] {
	return file.duplicates.map((duplicate) =>
		finding('error', 'duplicate-key-within-file', file.path, duplicate.key, {
			occurrences: duplicate.occurrences,
		}),
	);
}

function emptyFindings(file: Catalogue): Finding[] {
	return file.entries
		.filter((entry) => entry.text !== undefined && isBlank(entry.text))
		.map((entry) => finding('warning', 'empty-value', file.path, entry.key));
}

function against(
	reference: Catalogue,
	target: Catalogue,
	library: Library,
	options: AuditOptions,
	findings: Finding[],
): void {
	findings.push(...duplicateFindings(target));
	// A key whose text is already a convention finding is not compared further:
	// a count difference on top would be the same defect twice.
	const conventions = conventionFindings(target, options);
	const mismatched = new Set(conventions.map((found) => found.key));
	findings.push(...conventions);

	const index = new Map<string, Entry>();
	for (const entry of target.entries) index.set(entry.key, entry);

	// Structure first, and everything below a divergence is suppressed.
	const diverged = structureFindings(reference, index, target, findings);

	const fold = library.plurals === 'key-suffix';
	const sourceBases = bases(reference, fold);
	const targetBases = bases(target, fold);
	findings.push(
		...absent(
			reference,
			targetBases,
			diverged,
			fold,
			target.path,
			'missing-key',
		),
	);
	findings.push(
		...absent(target, sourceBases, diverged, fold, target.path, 'extra-key'),
	);

	for (const entry of reference.entries) {
		const source = sourceText(entry, options.keysAreSource);
		if (source === undefined) continue;
		const targetText = index.get(entry.key)?.text;
		if (targetText === undefined) continue;
		// An empty translation is already the finding.
		if (isBlank(targetText)) continue;
		if (!mismatched.has(entry.key)) {
			const placeholder = placeholderFinding(
				entry.key,
				source,
				targetText,
				library,
				target.path,
			);
			if (placeholder) {
				findings.push(placeholder);
				continue;
			}
		}
		if (source !== '' && source === targetText) {
			findings.push(finding('info', 'untranslated', target.path, entry.key));
		}
	}

	findings.push(...emptyFindings(target));
}

/** Every path a file defines, reduced to its plural base. Objects count. */
function bases(file: Catalogue, fold: boolean): Set<string> {
	return new Set(file.entries.map((entry) => baseOf(entry.key, fold)));
}

/** Keys of `file` whose base is not in `present`, one finding per base. */
function absent(
	file: Catalogue,
	present: ReadonlySet<string>,
	diverged: readonly string[],
	fold: boolean,
	reportedAs: string,
	kind: Kind,
): Finding[] {
	const seen = new Set<string>();
	const out: Finding[] = [];
	for (const entry of file.entries) {
		if (entry.shape === 'object' || isBelow(entry.key, diverged)) continue;
		const base = baseOf(entry.key, fold);
		if (present.has(base) || seen.has(base)) continue;
		seen.add(base);
		out.push(finding('error', kind, reportedAs, base));
	}
	return out;
}

function baseOf(key: string, fold: boolean): string {
	return fold ? pluralBase(key) : key;
}

function structureFindings(
	reference: Catalogue,
	index: ReadonlyMap<string, Entry>,
	target: Catalogue,
	findings: Finding[],
): string[] {
	const diverged: string[] = [];
	for (const entry of reference.entries) {
		const other = index.get(entry.key);
		if (!other || other.shape === entry.shape) continue;
		diverged.push(entry.key);
		findings.push(
			finding('error', 'structure-mismatch', target.path, entry.key, {
				sourceShape: entry.shape,
				targetShape: other.shape,
			}),
		);
	}
	return diverged;
}

function isBelow(key: string, diverged: readonly string[]): boolean {
	return diverged.some(
		(prefix) => key.startsWith(prefix) && key.charAt(prefix.length) === '.',
	);
}

/**
 * The one placeholder finding a key deserves: style before count before names,
 * so one mistake is never reported twice.
 */
function placeholderFinding(
	key: string,
	source: string,
	target: string,
	library: Library,
	file: string,
): Finding | undefined {
	const grammar = library.grammar;
	const sourceParsed = parseMessage(grammar, source);
	const targetParsed = parseMessage(grammar, target);

	const other = targetParsed.others[0];
	if (sourceParsed.tokens.length > 0 && other !== undefined) {
		return finding('error', 'placeholder-style-mismatch', file, key, {
			sourceStyle: grammar.interpolation,
			targetStyle: other,
		});
	}
	if (sourceParsed.tokens.length !== targetParsed.tokens.length) {
		return finding('error', 'placeholder-count-mismatch', file, key, {
			sourceCount: sourceParsed.tokens.length,
			targetCount: targetParsed.tokens.length,
		});
	}
	const sourceNames = names(sourceParsed.tokens);
	const targetNames = names(targetParsed.tokens);
	if (sourceNames.join('\u0000') === targetNames.join('\u0000'))
		return undefined;
	return finding('error', 'placeholder-name-mismatch', file, key, {
		sourceTokens: sourceNames,
		targetTokens: targetNames,
	});
}

/** Sorted and deduplicated. Token names are ASCII, so code-unit order is byte order. */
function names(tokens: readonly string[]): string[] {
	return [...new Set(tokens)].sort();
}

/** Which text of an entry is the English one. */
function sourceText(entry: Entry, keysAreSource: boolean): string | undefined {
	if (entry.shape !== 'text') return undefined;
	return keysAreSource ? entry.key : entry.text;
}
