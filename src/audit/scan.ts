import type { FileSystem } from './fs';
import { identify } from './identify';
import { readText } from './layout';
import type { LibraryId } from './library';
import {
	type CatalogueDocument,
	type Diagnostic,
	type Report,
	reportFor,
} from './report';

/**
 * One audit end to end over a filesystem — the crate's `scan::scan`.
 *
 * **Identify, then read, then audit.** Nothing reaches the audit without a
 * library in hand. One unreadable catalogue does not stop the rest; it is
 * named, so the answer is never quietly narrower than it looks.
 */

export interface ScanRequest {
	readonly fs: FileSystem;
	/** Directories or files, as POSIX paths. */
	readonly inputs: readonly string[];
	readonly named: LibraryId | undefined;
	readonly source: string | undefined;
	readonly keysAreSource: boolean;
	/** How a refusal tells the reader to name the library. */
	readonly howToName: string;
}

export async function scan(request: ScanRequest): Promise<Report> {
	const identified = await identify({
		fs: request.fs,
		inputs: request.inputs,
		named: request.named,
		howToName: request.howToName,
	});

	const documents: CatalogueDocument[] = [];
	const skipped: Diagnostic[] = [];
	for (const file of identified.files) {
		try {
			documents.push({
				name: file.name,
				locale: file.locale,
				content: await readText(request.fs, file.path),
			});
		} catch (error) {
			skipped.push({
				severity: 'warning',
				code: 'skipped',
				file: file.name,
				message: error instanceof Error ? error.message : String(error),
			});
		}
	}

	const report = reportFor(
		documents,
		{
			library: identified.library,
			version: identified.version,
			layout: identified.shape,
			keysAreSource: request.keysAreSource || identified.keysAreSource,
			evidence: identified.evidence,
		},
		{ source: request.source, keysAreSource: request.keysAreSource },
	);
	return { ...report, diagnostics: [...skipped, ...report.diagnostics] };
}
