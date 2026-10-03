import { parseCatalogue } from './catalogue';
import { ancestors, basename, byPath, type FileSystem, join, utf8 } from './fs';
import { type Node, parseJson } from './json';
import {
	ANCESTORS,
	cataloguesIn,
	describeShape,
	type Located,
	readLayout,
	readSet,
} from './layout';
import {
	configMatches,
	type Layout,
	type LayoutShape,
	LIBRARIES,
	type Library,
	type LibraryId,
	libraryOf,
	type ManifestKind,
	type Mark,
	packageOf,
} from './library';
import { keyMarks, marks } from './message';
import { lines, startsWithWhitespace, trim, trimEnd } from './text';

/**
 * The front door: which i18n library does this project use? — the crate's
 * `identify.rs`.
 *
 * **Corroboration, not inference.** Five classes of evidence — manifest,
 * config, layout, content, call site — and two agreeing is an identification.
 * A decisive content mark identifies alone. Two libraries identified is a
 * refusal naming both; none is a refusal naming every signal found.
 *
 * **Source files are read here, and only to answer which library this is.**
 * The scan looks for fixed substrings, keeps nothing, and reports only the
 * path. No finding ever comes from a source file.
 */

const MAX_SOURCE_FILES = 400;
const MAX_SOURCE_BYTES = 512 * 1024;
const MAX_SOURCE_DEPTH = 6;
const MAX_SAMPLED = 12;
const MAX_MANIFEST_BYTES = 8 * 1024 * 1024;

const SKIPPED_DIRECTORIES: ReadonlySet<string> = new Set([
	'node_modules',
	'.git',
	'target',
	'dist',
	'build',
	'.next',
	'out',
	'out-test',
	'coverage',
	'vendor',
	'lib',
]);

const SOURCE_EXTENSIONS: ReadonlySet<string> = new Set([
	'ts',
	'tsx',
	'js',
	'jsx',
	'mjs',
	'cjs',
	'vue',
	'svelte',
	'dart',
]);

const SECTIONS = [
	'dependencies',
	'devDependencies',
	'peerDependencies',
] as const;

export type SignalClass =
	| 'manifest'
	| 'config'
	| 'layout'
	| 'content'
	| 'call-site';

export interface Signal {
	readonly class: SignalClass;
	readonly library: LibraryId;
	/** A path relative to the catalogues, a package name, a mark — never file content. */
	readonly detail: string;
}

export interface Identified {
	readonly library: LibraryId;
	readonly version: string | null;
	readonly shape: LayoutShape;
	readonly keysAreSource: boolean;
	readonly evidence: readonly Signal[];
	readonly files: readonly Located[];
}

/**
 * How a refusal tells the reader to name the library: the CLI says "with
 * --system <name>", and the extension names its setting. Everything else in
 * the wording is the crate's.
 */
export interface Identification {
	readonly fs: FileSystem;
	readonly inputs: readonly string[];
	/** The library a caller named, which skips detection and the version gate. */
	readonly named: LibraryId | undefined;
	readonly howToName: string;
}

export async function identify(request: Identification): Promise<Identified> {
	const { fs, inputs } = request;
	const anchor = await anchorOf(fs, inputs);
	const evidence = await gather(fs, anchor, inputs);

	let library = request.named;
	if (library === undefined) {
		library = decide(evidence, request.howToName);
		await checkVersion(fs, anchor, library);
	}

	const reading = await readSet(fs, inputs, anchor, library);
	return {
		library,
		version: (await declaredVersion(fs, anchor, library)) ?? null,
		shape: reading.shape,
		keysAreSource: reading.keysAreSource,
		evidence: evidence.filter((signal) => signal.library === library),
		files: reading.files,
	};
}

async function anchorOf(
	fs: FileSystem,
	inputs: readonly string[],
): Promise<string> {
	const first = inputs[0];
	if (first === undefined)
		throw new Error('name the directory holding the catalogues');
	const metadata = await fs.stat(first);
	if (!metadata) throw new Error(`${first}: No such file or directory`);
	if (metadata.type === 'directory') return first;
	const at = first.lastIndexOf('/');
	return at <= 0 ? '/' : first.slice(0, at);
}

// ---------------------------------------------------------------------------
// The decision
// ---------------------------------------------------------------------------

/** Two agreeing classes, or one decisive signature. Never pick between two answers. */
function decide(evidence: readonly Signal[], howToName: string): LibraryId {
	const decisive = LIBRARIES.filter((library) =>
		library.signatures
			.filter((signature) => signature.strength === 'decisive')
			.some((signature) =>
				carries(evidence, library.id, 'content', signature.mark),
			),
	).map((library) => library.id);
	if (decisive.length === 1) return decisive[0] as LibraryId;

	const identified = LIBRARIES.map((library) => library.id).filter(
		(id) => classesFor(evidence, id) >= 2,
	);
	const candidates = decisive.length === 0 ? identified : decisive;
	if (candidates.length === 1) return candidates[0] as LibraryId;
	if (candidates.length === 0)
		throw new Error(unidentified(evidence, howToName));
	throw new Error(conflicted(evidence, candidates, howToName));
}

function classesFor(evidence: readonly Signal[], id: LibraryId): number {
	return new Set(
		evidence
			.filter((signal) => signal.library === id)
			.map((signal) => signal.class),
	).size;
}

function carries(
	evidence: readonly Signal[],
	id: LibraryId,
	kind: SignalClass,
	mark: Mark,
): boolean {
	return evidence.some(
		(signal) =>
			signal.library === id && signal.class === kind && signal.detail === mark,
	);
}

function unidentified(evidence: readonly Signal[], howToName: string): string {
	if (evidence.length === 0) {
		return `no i18n library could be identified here — nothing in the manifests, the config files, the layout, the catalogue syntax or the call sites points at one. Name it ${howToName}.`;
	}
	return `no i18n library could be identified here. Found, but not enough to agree: ${listed(evidence)}. Two classes of evidence are needed. Name it ${howToName}.`;
}

function conflicted(
	evidence: readonly Signal[],
	several: readonly LibraryId[],
	howToName: string,
): string {
	const each = several.map((id) => {
		const forThis = evidence.filter((signal) => signal.library === id);
		return `${id} (${forThis.map(describe).join(', ')})`;
	});
	return `${several.length} libraries are identified here and only one can be right: ${each.join('; ')}. Name the one these catalogues belong to ${howToName}.`;
}

function listed(evidence: readonly Signal[]): string {
	return evidence
		.map((signal) => `${signal.library} ${describe(signal)}`)
		.join(', ');
}

const CLASS_NAMES: Readonly<Record<SignalClass, string>> = Object.freeze({
	manifest: 'manifest',
	config: 'config',
	layout: 'layout',
	content: 'content',
	'call-site': 'call site',
});

function describe(signal: Signal): string {
	return `${CLASS_NAMES[signal.class]}: ${signal.detail}`;
}

// ---------------------------------------------------------------------------
// Gathering
// ---------------------------------------------------------------------------

async function gather(
	fs: FileSystem,
	anchor: string,
	inputs: readonly string[],
): Promise<Signal[]> {
	return [
		...(await manifestSignals(fs, anchor)),
		...(await configSignals(fs, anchor)),
		...(await layoutSignals(fs, anchor, inputs)),
		...(await contentSignals(fs, anchor, inputs)),
		...(await callSignals(fs, anchor)),
	];
}

/** A path relative to the catalogues, never absolute. */
function shown(path: string, anchor: string): string {
	const up = ancestors(anchor, Number.MAX_SAFE_INTEGER);
	for (const [count, ancestor] of up.entries()) {
		const prefix = ancestor === '/' ? '/' : `${ancestor}/`;
		if (path === ancestor || path.startsWith(prefix)) {
			const rest = path === ancestor ? '' : path.slice(prefix.length);
			return `${'../'.repeat(count)}${rest}`;
		}
	}
	return basename(path);
}

async function manifestSignals(
	fs: FileSystem,
	anchor: string,
): Promise<Signal[]> {
	const signals: Signal[] = [];
	for (const directory of ancestors(anchor, ANCESTORS)) {
		signals.push(
			...(await npmSignals(fs, join(directory, 'package.json'), anchor)),
		);
		signals.push(
			...(await pubspecSignals(fs, join(directory, 'pubspec.yaml'), anchor)),
		);
	}
	return signals;
}

async function npmSignals(
	fs: FileSystem,
	path: string,
	anchor: string,
): Promise<Signal[]> {
	const manifest = await readJson(fs, path);
	if (!manifest) return [];
	const at = shown(path, anchor);
	const signals: Signal[] = [];
	for (const section of SECTIONS) {
		for (const [name] of members(manifest, section)) {
			for (const library of LIBRARIES) {
				if (packageOf(library, name, 'npm')) {
					signals.push({
						class: 'manifest',
						library: library.id,
						detail: `${name} in ${at}`,
					});
				}
			}
		}
	}
	// VS Code declares its bundle directory in the manifest rather than
	// depending on a package: the mechanism is the editor.
	if (field(manifest, 'l10n') !== undefined) {
		signals.push({
			class: 'manifest',
			library: 'vscode-l10n',
			detail: `the l10n field in ${at}`,
		});
	}
	return signals;
}

async function pubspecSignals(
	fs: FileSystem,
	path: string,
	anchor: string,
): Promise<Signal[]> {
	const text = await readRegular(fs, path, MAX_MANIFEST_BYTES);
	if (text === undefined) return [];
	const at = shown(path, anchor);
	const signals: Signal[] = [];
	for (const [name] of pubspecDependencies(text)) {
		for (const library of LIBRARIES) {
			if (packageOf(library, name, 'pubspec')) {
				signals.push({
					class: 'manifest',
					library: library.id,
					detail: `${name} in ${at}`,
				});
			}
		}
	}
	return signals;
}

/** A line scan, not a YAML parser: the indented keys under `dependencies:`. */
function pubspecDependencies(text: string): Array<[string, string]> {
	const found: Array<[string, string]> = [];
	let inside = false;
	for (const line of lines(text)) {
		const trimmed = trimEnd(line);
		if (!startsWithWhitespace(trimmed) && trimmed !== '') {
			inside = trimmed === 'dependencies:' || trimmed === 'dev_dependencies:';
			continue;
		}
		if (!inside) continue;
		const entry = trim(trimmed);
		const colon = entry.indexOf(':');
		if (colon === -1) continue;
		found.push([trim(entry.slice(0, colon)), trim(entry.slice(colon + 1))]);
	}
	return found;
}

async function configSignals(
	fs: FileSystem,
	anchor: string,
): Promise<Signal[]> {
	const signals: Signal[] = [];
	for (const directory of ancestors(anchor, ANCESTORS)) {
		const entries = [...(await fs.readDirectory(directory))].sort(([a], [b]) =>
			byPath(a, b),
		);
		for (const [name, type] of entries) {
			if (type !== 'file') continue;
			// A config is a file with an extension: a directory called `l10n` is not it.
			const dot = name.lastIndexOf('.');
			if (dot === -1) continue;
			const stem = name.slice(0, dot);
			const extension = name.slice(dot + 1);
			for (const library of LIBRARIES) {
				if (
					library.configs.some((config) =>
						configMatches(config, stem, extension),
					)
				) {
					signals.push({
						class: 'config',
						library: library.id,
						detail: shown(join(directory, name), anchor),
					});
				}
			}
		}
	}
	return signals;
}

async function layoutSignals(
	fs: FileSystem,
	anchor: string,
	inputs: readonly string[],
): Promise<Signal[]> {
	const here = basename(anchor);
	const signals: Signal[] = [];
	for (const library of LIBRARIES) {
		for (const layout of library.layouts) {
			if (!distinctive(layout, here)) continue;
			let reads = false;
			try {
				reads = (await readLayout(fs, inputs, anchor, layout)) !== undefined;
			} catch {
				// A directory that does not fit this layout votes nothing rather
				// than refusing: evidence gathering never fails the run.
				reads = false;
			}
			if (reads)
				signals.push({
					class: 'layout',
					library: library.id,
					detail: describeShape(layout.shape),
				});
		}
	}
	return signals;
}

/** A layout votes only when it is one library's shape, or from a directory it names. */
function distinctive(layout: Layout, here: string): boolean {
	if (layout.directories.length > 0) return layout.directories.includes(here);
	return !(
		layout.shape.shape === 'shared' && layout.shape.extension === 'json'
	);
}

async function contentSignals(
	fs: FileSystem,
	anchor: string,
	inputs: readonly string[],
): Promise<Signal[]> {
	const found = await sampledMarks(fs, anchor, inputs);
	const signals: Signal[] = [];
	for (const library of LIBRARIES) {
		for (const signature of library.signatures) {
			// A weak mark is recorded for nothing: `{name}` is prose everywhere.
			if (signature.strength === 'weak' || !found.includes(signature.mark))
				continue;
			signals.push({
				class: 'content',
				library: library.id,
				detail: signature.mark,
			});
		}
	}
	return signals;
}

async function sampledMarks(
	fs: FileSystem,
	anchor: string,
	inputs: readonly string[],
): Promise<Mark[]> {
	const found: Mark[] = [];
	const note = (mark: Mark): void => {
		if (!found.includes(mark)) found.push(mark);
	};
	for (const path of await sample(fs, anchor, inputs)) {
		let text: string | undefined;
		try {
			text = utf8(await fs.readFile(path));
		} catch {
			// Unreadable: no content evidence from this file, as in the crate.
			text = undefined;
		}
		if (text === undefined) continue;
		let parsed: ReturnType<typeof parseCatalogue>;
		try {
			parsed = parseCatalogue(text);
		} catch {
			// Unparsable: it is read for evidence only, and the audit names it.
			continue;
		}
		keyMarks(parsed.entries.map((entry) => entry.key)).forEach(note);
		for (const entry of parsed.entries) {
			if (entry.text !== undefined) marks(entry.text).forEach(note);
		}
	}
	return found;
}

async function sample(
	fs: FileSystem,
	anchor: string,
	inputs: readonly string[],
): Promise<string[]> {
	const found: string[] = [];
	for (const input of inputs) {
		if ((await fs.stat(input))?.type === 'file') found.push(input);
	}
	found.push(...(await cataloguesIn(fs, anchor)));
	if (found.length === 0) {
		// The namespaced layout keeps its catalogues one level down.
		for (const [name, type] of await fs.readDirectory(anchor)) {
			if (type === 'directory')
				found.push(...(await cataloguesIn(fs, join(anchor, name))));
		}
	}
	return [...new Set(found)].sort(byPath).slice(0, MAX_SAMPLED);
}

async function callSignals(fs: FileSystem, anchor: string): Promise<Signal[]> {
	const root = await projectRoot(fs, anchor);
	const signals: Signal[] = [];
	const budget = { remaining: MAX_SOURCE_FILES };
	await walkSource(fs, root, 0, budget, (path, text) => {
		for (const library of LIBRARIES) {
			if (
				signals.some(
					(signal) =>
						signal.library === library.id && signal.class === 'call-site',
				)
			)
				continue;
			const call = library.calls.find((candidate) => text.includes(candidate));
			if (call !== undefined) {
				signals.push({
					class: 'call-site',
					library: library.id,
					detail: `${call} in ${shown(path, anchor)}`,
				});
			}
		}
	});
	return signals;
}

async function projectRoot(fs: FileSystem, anchor: string): Promise<string> {
	for (const directory of ancestors(anchor, ANCESTORS)) {
		if ((await fs.stat(join(directory, 'package.json')))?.type === 'file')
			return directory;
		if ((await fs.stat(join(directory, 'pubspec.yaml')))?.type === 'file')
			return directory;
	}
	return anchor;
}

async function walkSource(
	fs: FileSystem,
	directory: string,
	depth: number,
	budget: { remaining: number },
	visit: (path: string, text: string) => void,
): Promise<void> {
	if (depth > MAX_SOURCE_DEPTH || budget.remaining === 0) return;
	const entries = [...(await fs.readDirectory(directory))].sort(([a], [b]) =>
		byPath(a, b),
	);
	const directories: string[] = [];
	for (const [name, type] of entries) {
		const path = join(directory, name);
		if (type === 'directory') {
			if (!name.startsWith('.') && !SKIPPED_DIRECTORIES.has(name))
				directories.push(path);
			continue;
		}
		const dot = name.lastIndexOf('.');
		const extension = dot > 0 ? name.slice(dot + 1) : '';
		if (!SOURCE_EXTENSIONS.has(extension) || budget.remaining === 0) continue;
		const text = await readRegular(fs, path, MAX_SOURCE_BYTES);
		if (text === undefined) continue;
		budget.remaining--;
		visit(path, text);
	}
	for (const child of directories)
		await walkSource(fs, child, depth + 1, budget, visit);
}

/** A regular file no larger than `cap`, as text — or nothing. */
async function readRegular(
	fs: FileSystem,
	path: string,
	cap: number,
): Promise<string | undefined> {
	const metadata = await fs.stat(path);
	if (metadata?.type !== 'file' || metadata.size > cap) return undefined;
	try {
		return utf8(await fs.readFile(path));
	} catch {
		// Somebody else's file that will not open is no evidence, never a refusal.
		return undefined;
	}
}

/** A manifest that will not parse is somebody else's broken file, not a refusal. */
async function readJson(
	fs: FileSystem,
	path: string,
): Promise<Node | undefined> {
	const text = await readRegular(fs, path, MAX_MANIFEST_BYTES);
	if (text === undefined) return undefined;
	try {
		return parseJson(text);
	} catch {
		// serde_json's refusal, so the same manifests are ignored on both sides.
		return undefined;
	}
}

/** An object's last value for a key, as `serde_json::Value` keeps it. */
function field(node: Node, key: string): Node | undefined {
	if (node.kind !== 'object') return undefined;
	let found: Node | undefined;
	for (const [name, value] of node.entries) if (name === key) found = value;
	return found;
}

/** An object section's members, last value per key, in `serde_json::Map`'s sorted order. */
function members(manifest: Node, section: string): Array<[string, Node]> {
	const object = field(manifest, section);
	if (object?.kind !== 'object') return [];
	const last = new Map<string, Node>();
	for (const [name, value] of object.entries) last.set(name, value);
	return [...last].sort(([a], [b]) => byPath(a, b));
}

// ---------------------------------------------------------------------------
// Version
// ---------------------------------------------------------------------------

async function declaredVersion(
	fs: FileSystem,
	anchor: string,
	library: LibraryId,
): Promise<string | undefined> {
	const row = libraryOf(library);
	for (const directory of ancestors(anchor, ANCESTORS)) {
		const found =
			(await npmVersion(fs, join(directory, 'package.json'), row)) ??
			(await pubspecVersion(fs, join(directory, 'pubspec.yaml'), row));
		if (found !== undefined) return found;
	}
	return undefined;
}

async function npmVersion(
	fs: FileSystem,
	path: string,
	row: Library,
): Promise<string | undefined> {
	const manifest = await readJson(fs, path);
	if (!manifest) return undefined;
	for (const section of SECTIONS) {
		for (const [name, spec] of members(manifest, section)) {
			if (!packageOf(row, name, 'npm')) continue;
			if (spec.kind === 'text' && spec.text !== '') return spec.text;
		}
	}
	return undefined;
}

async function pubspecVersion(
	fs: FileSystem,
	path: string,
	row: Library,
): Promise<string | undefined> {
	const text = await readRegular(fs, path, MAX_MANIFEST_BYTES);
	if (text === undefined) return undefined;
	const found = pubspecDependencies(text).find(
		([name, version]) =>
			packageOf(row, name, 'pubspec' as ManifestKind) && version !== '',
	);
	return found?.[1];
}

/** The only version gate: a major below which the library wrote its catalogues differently. */
async function checkVersion(
	fs: FileSystem,
	anchor: string,
	library: LibraryId,
): Promise<void> {
	const row = libraryOf(library);
	for (const directory of ancestors(anchor, ANCESTORS)) {
		const path = join(directory, 'package.json');
		const manifest = await readJson(fs, path);
		if (!manifest) continue;
		for (const section of SECTIONS) {
			for (const [name, spec] of members(manifest, section)) {
				const known = packageOf(row, name, 'npm');
				if (!known || known.breakingBelow === undefined) continue;
				const text = spec.kind === 'text' ? spec.text : undefined;
				const major = text === undefined ? undefined : majorOf(text);
				if (major === undefined || major >= known.breakingBelow) continue;
				throw new Error(
					`${name} ${text} in ${shown(path, anchor)} writes its catalogues differently from ${known.breakingBelow} and later, which is the model this reads. Auditing it as ${library} would be wrong.`,
				);
			}
		}
	}
}

function majorOf(spec: string): number | undefined {
	let version = spec;
	if (spec.startsWith('npm:')) {
		const alias = spec.slice('npm:'.length);
		const at = alias.lastIndexOf('@');
		if (at === -1) return undefined;
		version = alias.slice(at + 1);
	}
	const digits = /^[^0-9]*([0-9]*)/.exec(version)?.[1] ?? '';
	if (digits === '') return undefined;
	// `u64::parse` refuses a number too large for 64 bits.
	if (BigInt(digits) > 0xffff_ffff_ffff_ffffn) return undefined;
	return Number(digits);
}
