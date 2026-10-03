/**
 * What each i18n library **is**, as data — the crate's `library.rs`.
 *
 * Identification decides which row applies; the grammar, the plural model,
 * which keys are metadata and how the files are laid out are all read off that
 * row. Nothing downstream guesses. The rows and their order are the crate's,
 * because a refusal lists the libraries in this order and both surfaces must
 * list them the same way.
 */

export type LibraryId = 'i18next' | 'next-intl' | 'vscode-l10n' | 'flutter-arb';

export type Interpolation = 'double-brace' | 'single-brace' | 'positional';

export type Mark =
	| 'double-brace'
	| 'icu-argument'
	| 'positional'
	| 'single-brace'
	| 'dollar-t'
	| 'plural-key-suffix'
	| 'arb-metadata'
	| 'sentence-keys';

export type Strength = 'decisive' | 'strong' | 'weak';

export type ManifestKind = 'npm' | 'pubspec';

export interface Grammar {
	readonly interpolation: Interpolation;
	/** `{count, plural, ...}` is native here. */
	readonly icu: boolean;
	/** `$t(other.key)` is a key reference here. */
	readonly nesting: boolean;
	/** `{{ name }}` and `{{name}}` are the same variable. */
	readonly trims: boolean;
}

export type LayoutShape =
	| { readonly shape: 'shared'; readonly extension: string }
	| {
			readonly shape: 'fixed';
			readonly prefix: string;
			readonly extension: string;
	  }
	| { readonly shape: 'namespaced'; readonly extension: string };

export interface Layout {
	readonly shape: LayoutShape;
	/** Directory names this library writes. Empty means any. */
	readonly directories: readonly string[];
	/** The key is the English string, as in `bundle.l10n.json`. */
	readonly keysAreSource: boolean;
}

export interface Package {
	readonly name: string;
	readonly manifest: ManifestKind;
	/** A major below which this library's model is known to be different. */
	readonly breakingBelow: number | undefined;
}

export interface Config {
	readonly stem: string;
	readonly extensions: readonly string[];
}

export interface Library {
	readonly id: LibraryId;
	readonly packages: readonly Package[];
	readonly configs: readonly Config[];
	/** Identification only: source is never read for a finding. */
	readonly calls: readonly string[];
	readonly layouts: readonly Layout[];
	readonly signatures: ReadonlyArray<{
		readonly mark: Mark;
		readonly strength: Strength;
	}>;
	readonly grammar: Grammar;
	readonly plurals: 'key-suffix' | 'icu' | 'none';
	readonly metadata: 'double-at' | 'arb';
}

const JS_CONFIG = Object.freeze([
	'js',
	'cjs',
	'mjs',
	'ts',
	'mts',
	'cts',
	'json',
]);

export const LIBRARIES: readonly Library[] = Object.freeze([
	{
		id: 'i18next',
		packages: [
			{ name: 'i18next', manifest: 'npm', breakingBelow: 23 },
			{ name: 'react-i18next', manifest: 'npm', breakingBelow: undefined },
			{ name: 'i18next-vue', manifest: 'npm', breakingBelow: undefined },
		],
		configs: [
			{ stem: 'i18next-parser.config', extensions: JS_CONFIG },
			{ stem: 'next-i18next.config', extensions: JS_CONFIG },
		],
		calls: ['useTranslation(', 'i18next.t('],
		layouts: [
			{
				shape: { shape: 'namespaced', extension: 'json' },
				directories: [],
				keysAreSource: false,
			},
			{
				shape: { shape: 'shared', extension: 'json' },
				directories: ['locales', 'translations', 'lang'],
				keysAreSource: false,
			},
		],
		signatures: [
			{ mark: 'double-brace', strength: 'strong' },
			{ mark: 'dollar-t', strength: 'strong' },
			{ mark: 'plural-key-suffix', strength: 'strong' },
		],
		grammar: {
			interpolation: 'double-brace',
			icu: false,
			nesting: true,
			trims: true,
		},
		plurals: 'key-suffix',
		metadata: 'double-at',
	},
	{
		id: 'next-intl',
		packages: [
			{ name: 'next-intl', manifest: 'npm', breakingBelow: undefined },
		],
		configs: [{ stem: 'next-intl.config', extensions: JS_CONFIG }],
		calls: ['useTranslations(', 'getTranslations(', 'next-intl'],
		layouts: [
			{
				shape: { shape: 'shared', extension: 'json' },
				directories: ['messages'],
				keysAreSource: false,
			},
		],
		signatures: [
			{ mark: 'icu-argument', strength: 'strong' },
			{ mark: 'single-brace', strength: 'weak' },
		],
		grammar: {
			interpolation: 'single-brace',
			icu: true,
			nesting: false,
			trims: true,
		},
		plurals: 'icu',
		metadata: 'double-at',
	},
	{
		id: 'vscode-l10n',
		packages: [
			{ name: '@vscode/l10n', manifest: 'npm', breakingBelow: undefined },
		],
		configs: [],
		calls: ['vscode.l10n.t(', 'l10n.t('],
		layouts: [
			{
				shape: { shape: 'fixed', prefix: 'bundle.l10n', extension: 'json' },
				directories: [],
				keysAreSource: true,
			},
			{
				shape: { shape: 'fixed', prefix: 'package.nls', extension: 'json' },
				directories: [],
				keysAreSource: false,
			},
		],
		signatures: [
			{ mark: 'positional', strength: 'weak' },
			{ mark: 'sentence-keys', strength: 'strong' },
		],
		grammar: {
			interpolation: 'positional',
			icu: false,
			nesting: false,
			trims: false,
		},
		plurals: 'none',
		metadata: 'double-at',
	},
	{
		id: 'flutter-arb',
		packages: [
			{
				name: 'flutter_localizations',
				manifest: 'pubspec',
				breakingBelow: undefined,
			},
			{ name: 'intl', manifest: 'pubspec', breakingBelow: undefined },
		],
		configs: [{ stem: 'l10n', extensions: ['yaml', 'yml'] }],
		calls: ['AppLocalizations.of('],
		layouts: [
			{
				shape: { shape: 'shared', extension: 'arb' },
				directories: [],
				keysAreSource: false,
			},
		],
		signatures: [
			{ mark: 'arb-metadata', strength: 'decisive' },
			{ mark: 'icu-argument', strength: 'weak' },
		],
		grammar: {
			interpolation: 'single-brace',
			icu: true,
			nesting: false,
			trims: true,
		},
		plurals: 'icu',
		metadata: 'arb',
	},
] satisfies Library[]);

export function libraryNames(): string[] {
	return LIBRARIES.map((library) => library.id);
}

export function parseLibrary(name: string): LibraryId | undefined {
	return LIBRARIES.find((library) => library.id === name)?.id;
}

export function libraryOf(id: LibraryId): Library {
	return LIBRARIES.find((library) => library.id === id) as Library;
}

/** Whether a key is metadata, judged on its first dotted segment. */
export function isMetadata(library: Library, key: string): boolean {
	const head = key.split('.')[0] as string;
	return library.metadata === 'arb'
		? head.startsWith('@')
		: head.startsWith('@@');
}

export function configMatches(
	config: Config,
	stem: string,
	extension: string,
): boolean {
	return config.stem === stem && config.extensions.includes(extension);
}

export function packageOf(
	library: Library,
	name: string,
	manifest: ManifestKind,
): Package | undefined {
	return library.packages.find(
		(candidate) => candidate.name === name && candidate.manifest === manifest,
	);
}
