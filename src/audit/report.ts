import {
	audit,
	type Catalogue,
	type Finding,
	keyCount,
	withoutMetadata,
} from './audit';
import { parseCatalogue } from './catalogue';
import type { Signal } from './identify';
import { type LayoutShape, type LibraryId, libraryOf } from './library';
import { canonicalise } from './locale';

/**
 * One audit over content already in hand — the crate's `scan::report_for`,
 * which both of its surfaces call.
 *
 * The report is one object for the whole set, and it has no timestamp: a report
 * is a thing to diff, and a clock would make every run differ for a reason
 * that is not the catalogues.
 */

/** 3 replaced the guessed profile with `system` and dropped per-file refusals. */
export const SCHEMA = 3;

export type Status = 'clean' | 'findings' | 'no-files';

export interface Diagnostic {
	readonly severity: 'warning';
	readonly code: 'skipped' | 'unparsable';
	readonly file: string;
	readonly message: string;
}

export interface FileSummary {
	readonly path: string;
	readonly locale: string | null;
	/** A count, never a list. */
	readonly keys: number;
}

export type { Signal } from './identify';

export interface System {
	readonly library: LibraryId;
	readonly version: string | null;
	/** `null` where nothing looked at a filesystem: the MCP surface. */
	readonly layout: LayoutShape | null;
	readonly keysAreSource: boolean;
	readonly evidence: readonly Signal[];
}

export interface Report {
	readonly schema: number;
	readonly status: Status;
	readonly system: System;
	readonly source: FileSummary | null;
	readonly files: readonly FileSummary[];
	readonly findings: readonly Finding[];
	readonly diagnostics: readonly Diagnostic[];
	readonly summary: { readonly files: number; readonly findings: number };
}

/** One catalogue as a surface supplies it. `locale` undefined is the base. */
export interface CatalogueDocument {
	readonly name: string;
	readonly locale: string | undefined;
	readonly content: string;
}

export interface ReportOptions {
	/** A path or a language tag. Absent means auto-detect. */
	readonly source: string | undefined;
	readonly keysAreSource: boolean;
}

/** Throws with the crate's wording when the source cannot be settled. */
export function reportFor(
	documents: readonly CatalogueDocument[],
	system: System,
	options: ReportOptions,
): Report {
	const library = libraryOf(system.library);
	const keysAreSource = system.keysAreSource || options.keysAreSource;

	const files: Catalogue[] = [];
	const diagnostics: Diagnostic[] = [];
	for (const document of documents) {
		try {
			const parsed = parseCatalogue(document.content);
			files.push(
				withoutMetadata(
					{
						path: document.name,
						locale: document.locale,
						entries: parsed.entries,
						duplicates: parsed.duplicates,
					},
					library,
				),
			);
		} catch (error) {
			diagnostics.push({
				severity: 'warning',
				code: 'unparsable',
				file: document.name,
				message: error instanceof Error ? error.message : String(error),
			});
		}
	}

	if (files.length === 0) {
		return {
			schema: SCHEMA,
			status: 'no-files',
			system,
			source: null,
			files: [],
			findings: [],
			diagnostics,
			summary: { files: 0, findings: 0 },
		};
	}

	const sourceIndex = resolveSource(files, options.source);
	const findings = audit(files, sourceIndex, {
		library: system.library,
		keysAreSource,
	});
	const summaries = files.map(summarise);
	return {
		schema: SCHEMA,
		status: findings.length === 0 ? 'clean' : 'findings',
		system,
		source: summaries[sourceIndex] ?? null,
		files: summaries,
		findings,
		diagnostics,
		summary: { files: summaries.length, findings: findings.length },
	};
}

function summarise(file: Catalogue): FileSummary {
	return { path: file.path, locale: file.locale ?? null, keys: keyCount(file) };
}

/**
 * Which catalogue is the contract. Auto-detection answers only when exactly one
 * candidate exists, and names the fix when there is not.
 */
function resolveSource(
	files: readonly Catalogue[],
	wanted: string | undefined,
): number {
	if (wanted === undefined) return autoSource(files);

	const byPath = files.findIndex((file) => file.path === wanted);
	if (byPath !== -1) return byPath;
	const tag = canonicalise(wanted);
	if (tag !== undefined) {
		const byLocale = files.findIndex((file) => file.locale === tag);
		if (byLocale !== -1) return byLocale;
	}
	throw new Error(
		`no catalogue here is called ${wanted}. Found: ${listed(files)}`,
	);
}

function autoSource(files: readonly Catalogue[]): number {
	const candidates = files.flatMap((file, index) =>
		isEnglish(file) ? [index] : [],
	);
	if (candidates.length === 1) return candidates[0] as number;
	if (candidates.length === 0) {
		throw new Error(
			`no English catalogue here, so say which one is the source. Found: ${listed(files)}`,
		);
	}
	throw new Error(
		`${candidates.length} catalogues could be the English one, so say which is the source: ${candidates
			.map((index) => (files[index] as Catalogue).path)
			.join(', ')}`,
	);
}

/** Tagged `en`-something, or untagged — which is how both VS Code layouts write English. */
function isEnglish(file: Catalogue): boolean {
	return file.locale === undefined || file.locale.split('-')[0] === 'en';
}

function listed(files: readonly Catalogue[]): string {
	return files.map((file) => file.path).join(', ');
}
