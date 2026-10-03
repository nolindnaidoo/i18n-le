import { type LibraryId, libraryNames, parseLibrary } from '../audit/library';
import { canonicalise, localesOf } from '../audit/locale';
import { type CatalogueDocument, reportFor } from '../audit/report';
import {
	capped,
	DEFAULT_MAX_RESULTS,
	type Diagnostic,
	envelope,
	MAX_MAX_RESULTS,
	readMaxResults,
} from './envelope';
import type { ToolDefinition } from './transport';

/**
 * The tool this server exposes: `check_catalogues`, which the crate's server
 * offers too. One name, one schema, two implementations — a caller must get
 * the same answer whichever it reaches, so the definition below is the
 * crate's, word for word, and `crate/fixtures/mcp-check-catalogues.json` pins
 * the answers on both sides.
 *
 * **The library is required, never detected.** Identification reads manifests,
 * config files, layouts and call sites, none of which exist on this surface.
 * **Only keys, tokens and counts come back** — never a translated string.
 */

const DESCRIPTION =
	'Audit a set of translation catalogues against one of them and report what is structurally wrong: missing and extra keys, placeholders dropped or renamed in translation, constructs from another i18n convention, empty values, keys defined twice, and a path that is an object in one locale and a string in another. Takes file contents directly and reads no files. Only key names and structural facts are returned — never a translated string. The i18n library must be named: identifying it needs manifests, config files and source call sites, none of which this surface can see, and guessing the placeholder grammar from content is exactly what this tool does not do.';

const INVALID_FILES =
	'files is required and must be a non-empty array of { path, content }';

function readLibrary(args: Record<string, unknown>): LibraryId {
	const names = libraryNames().join(', ');
	const named = args.library;
	if (typeof named !== 'string') {
		throw new Error(
			`library is required and must be one of ${names}. It cannot be worked out from file contents alone, and guessing it is what this tool exists not to do.`,
		);
	}
	const id = parseLibrary(named);
	if (id === undefined)
		throw new Error(
			`${named} is not a library this reads. Try one of ${names}.`,
		);
	return id;
}

/**
 * Every locale settled before the audit starts. A caller that gives all of them
 * is believed; one that leaves any out has all of them read from the names
 * together, because only the set says what a name means.
 */
function readFiles(args: Record<string, unknown>): CatalogueDocument[] {
	const items = args.files;
	if (!Array.isArray(items) || items.length === 0)
		throw new Error(INVALID_FILES);

	const names: string[] = [];
	const contents: string[] = [];
	const supplied: Array<string | undefined> = [];
	for (const item of items as unknown[]) {
		const record = (
			typeof item === 'object' && item !== null ? item : {}
		) as Record<string, unknown>;
		if (typeof record.path !== 'string' || typeof record.content !== 'string')
			throw new Error(INVALID_FILES);
		names.push(record.path);
		contents.push(record.content);
		if (typeof record.locale !== 'string') {
			supplied.push(undefined);
			continue;
		}
		const canonical = canonicalise(record.locale);
		if (canonical === undefined)
			throw new Error(`${record.locale} is not a language tag`);
		supplied.push(canonical);
	}

	const locales = supplied.every((locale) => locale !== undefined)
		? supplied
		: localesOf(names);
	return names.map((name, index) => ({
		name,
		locale: locales[index],
		content: contents[index] as string,
	}));
}

function check(args: Record<string, unknown>): Promise<unknown> {
	// The crate's order, so a call with two mistakes is refused for the same one.
	const library = readLibrary(args);
	const documents = readFiles(args);
	const maxResults = readMaxResults(args);
	const keysAreSource = args.keysAreSource === true;

	const report = reportFor(
		documents,
		{ library, version: null, layout: null, keysAreSource, evidence: [] },
		{
			source: typeof args.source === 'string' ? args.source : undefined,
			keysAreSource,
		},
	);

	// The flag matters more than the cap: a tool whose job is telling you what
	// is absent must never shorten its answer silently.
	const { items, truncated } = capped(report.findings, maxResults);
	const diagnostics: Diagnostic[] = report.diagnostics.map((diagnostic) => ({
		severity: diagnostic.severity,
		code: diagnostic.code,
		message: `${diagnostic.file}: ${diagnostic.message}`,
	}));

	return Promise.resolve(
		envelope(
			'check_catalogues',
			{ ...report, findings: items },
			items.length,
			diagnostics,
			truncated,
		),
	);
}

export const TOOLS: readonly ToolDefinition[] = Object.freeze([
	Object.freeze({
		name: 'check_catalogues',
		description: DESCRIPTION,
		inputSchema: {
			type: 'object',
			properties: {
				library: {
					type: 'string',
					enum: libraryNames(),
					description:
						'Which i18n library wrote these catalogues. It supplies the placeholder grammar, the plural model and which keys are metadata.',
				},
				files: {
					type: 'array',
					minItems: 1,
					description: 'The catalogues to audit. JSON, nested or flat.',
					items: {
						type: 'object',
						properties: {
							path: {
								type: 'string',
								description:
									'File name, e.g. "pt-BR.json" or "bundle.l10n.pt-br.json". Used to label findings and, when locale is absent, to work out which locale this is.',
							},
							locale: {
								type: 'string',
								description:
									'The language tag, e.g. "pt-BR". Omit for the base catalogue, or omit on every file to have it read from the names.',
							},
							content: { type: 'string', description: 'The file contents.' },
						},
						required: ['path', 'content'],
						additionalProperties: false,
					},
				},
				source: {
					type: 'string',
					description:
						'Which catalogue is the contract — a path or a language tag. Without it, exactly one English candidate must exist or the audit is refused rather than guessed at.',
				},
				keysAreSource: {
					type: 'boolean',
					default: false,
					description:
						'The key is itself the English string, as in a VS Code bundle.l10n.json.',
				},
				maxResults: {
					type: 'integer',
					minimum: 1,
					maximum: MAX_MAX_RESULTS,
					default: DEFAULT_MAX_RESULTS,
					description: `Cap on returned findings (default ${DEFAULT_MAX_RESULTS}). meta.truncated reports whether any were dropped.`,
				},
			},
			required: ['library', 'files'],
			additionalProperties: false,
		},
		handler: check,
	}),
]);
