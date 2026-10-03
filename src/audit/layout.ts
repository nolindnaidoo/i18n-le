import {
	ancestors,
	basename,
	byPath,
	dirname,
	extensionOf,
	type FileSystem,
	join,
	utf8,
} from './fs';
import {
	type Layout,
	type LayoutShape,
	type LibraryId,
	libraryOf,
} from './library';
import { canonicalise, fromPrefix, localesOf } from './locale';

/**
 * Which files the set is, and what locale each is, once the library is known —
 * the crate's `layout.rs`.
 *
 * - **shared** — the locale is whatever the names do not share, so the set
 *   lives in one directory.
 * - **fixed** — the library supplies the prefix, so one name reads alone and
 *   the set may span two directories: every VS Code extension's shape.
 * - **namespaced** — the parent directory is the locale. One namespace is one
 *   set.
 */

/** How far up this ever looks — for a manifest, a config, or a base file. */
export const ANCESTORS = 8;

export interface Located {
	readonly path: string;
	/** The name this file is reported under. */
	readonly name: string;
	readonly locale: string | undefined;
}

export interface Reading {
	readonly files: readonly Located[];
	readonly shape: LayoutShape;
	readonly keysAreSource: boolean;
}

/** The first of the library's layouts that reads the files in `inputs`. */
export async function readSet(
	fs: FileSystem,
	inputs: readonly string[],
	anchor: string,
	library: LibraryId,
): Promise<Reading> {
	const row = libraryOf(library);
	let refusal: string | undefined;
	for (const layout of row.layouts) {
		try {
			const reading = await readLayout(fs, inputs, anchor, layout);
			if (reading) return reading;
		} catch (error) {
			// Kept rather than thrown: another layout may still read these files.
			refusal = error instanceof Error ? error.message : String(error);
		}
	}
	if (refusal !== undefined) throw new Error(refusal);

	// A directory with no catalogues at all is not a malformed question.
	let empty = true;
	for (const layout of row.layouts) {
		if ((await collect(fs, inputs, layout.shape.extension)).length > 0)
			empty = false;
	}
	const first = row.layouts[0];
	if (empty && first)
		return {
			files: [],
			shape: first.shape,
			keysAreSource: first.keysAreSource,
		};

	throw new Error(
		`${library} was identified, but none of its layouts reads the files here (${row.layouts
			.map((layout) => describeShape(layout.shape))
			.join(', ')}).`,
	);
}

export async function readLayout(
	fs: FileSystem,
	inputs: readonly string[],
	anchor: string,
	layout: Layout,
): Promise<Reading | undefined> {
	const shape = layout.shape;
	switch (shape.shape) {
		case 'shared':
			return shared(fs, inputs, shape.extension, layout);
		case 'fixed':
			return fixed(fs, inputs, anchor, shape.prefix, shape.extension, layout);
		case 'namespaced':
			return namespaced(fs, inputs, shape.extension, layout);
	}
}

export function describeShape(shape: LayoutShape): string {
	switch (shape.shape) {
		case 'shared':
			return `a directory of <locale>.${shape.extension}`;
		case 'fixed':
			return `${shape.prefix}.<locale>.${shape.extension}`;
		case 'namespaced':
			return `<locale>/<namespace>.${shape.extension}`;
	}
}

async function shared(
	fs: FileSystem,
	inputs: readonly string[],
	extension: string,
	layout: Layout,
): Promise<Reading | undefined> {
	const files = await collect(fs, inputs, extension);
	if (files.length === 0) return undefined;
	oneDirectory(files);
	const names = files.map(basename);
	const locales = localesOf(names);
	return {
		files: zip(files, names, locales),
		shape: layout.shape,
		keysAreSource: layout.keysAreSource,
	};
}

async function fixed(
	fs: FileSystem,
	inputs: readonly string[],
	anchor: string,
	prefix: string,
	extension: string,
	layout: Layout,
): Promise<Reading | undefined> {
	const files = (await collect(fs, inputs, extension)).filter((path) =>
		basename(path).startsWith(prefix),
	);
	if (files.length === 0) return undefined;

	const base = `${prefix}.${extension}`;
	if (!files.some((path) => basename(path) === base)) {
		for (const directory of ancestors(anchor, ANCESTORS)) {
			const candidate = join(directory, base);
			if ((await fs.stat(candidate))?.type === 'file') {
				files.push(candidate);
				break;
			}
		}
	}
	const unique = [...new Set(files)].sort(byPath);
	const names = unique.map(basename);
	const locales = names.map((name) => fromPrefix(name, prefix));
	return {
		files: zip(unique, names, locales),
		shape: layout.shape,
		keysAreSource: layout.keysAreSource,
	};
}

async function namespaced(
	fs: FileSystem,
	inputs: readonly string[],
	extension: string,
	layout: Layout,
): Promise<Reading | undefined> {
	if (inputs.length !== 1) return undefined;
	const root = inputs[0] as string;
	if ((await fs.stat(root))?.type !== 'directory') return undefined;

	const directories = await localeDirectories(fs, root);
	if (!directories) return undefined;

	const namespaces: string[] = [];
	const files: string[] = [];
	for (const directory of directories) {
		for (const path of (await cataloguesIn(fs, directory)).filter(
			(p) => extensionOf(p) === extension,
		)) {
			const name = basename(path);
			if (!namespaces.includes(name)) namespaces.push(name);
			files.push(path);
		}
	}
	if (files.length === 0) return undefined;
	if (namespaces.length > 1) {
		namespaces.sort(byPath);
		throw new Error(
			`this set has ${namespaces.length} namespaces (${namespaces.join(', ')}) and a namespace is its own set. Name one namespace's files.`,
		);
	}
	return {
		files: files.map(inLocaleDirectory),
		shape: layout.shape,
		keysAreSource: layout.keysAreSource,
	};
}

/** The subdirectories of `root`, when every one of them is named for a locale. */
async function localeDirectories(
	fs: FileSystem,
	root: string,
): Promise<string[] | undefined> {
	const directories: string[] = [];
	for (const [name, type] of await fs.readDirectory(root)) {
		if (type !== 'directory') continue;
		if (canonicalise(name) === undefined) return undefined;
		directories.push(join(root, name));
	}
	if (directories.length === 0) return undefined;
	return directories.sort(byPath);
}

function inLocaleDirectory(path: string): Located {
	const tag = basename(dirname(path));
	return { path, name: `${tag}/${basename(path)}`, locale: canonicalise(tag) };
}

async function collect(
	fs: FileSystem,
	inputs: readonly string[],
	extension: string,
): Promise<string[]> {
	const files: string[] = [];
	for (const input of inputs) {
		if ((await fs.stat(input))?.type === 'file') {
			files.push(input);
			continue;
		}
		files.push(
			...(await cataloguesIn(fs, input)).filter(
				(path) => extensionOf(path) === extension,
			),
		);
	}
	return [...new Set(files)].sort(byPath);
}

/** The `.json` and `.arb` files directly inside one directory — never recursive. */
export async function cataloguesIn(
	fs: FileSystem,
	directory: string,
): Promise<string[]> {
	const found: string[] = [];
	for (const [name, type] of await fs.readDirectory(directory)) {
		if (type !== 'file') continue;
		const path = join(directory, name);
		const extension = extensionOf(path);
		if (extension === 'json' || extension === 'arb') found.push(path);
	}
	return found.sort(byPath);
}

function zip(
	files: readonly string[],
	names: readonly string[],
	locales: ReadonlyArray<string | undefined>,
): Located[] {
	return files.map((path, index) => ({
		path,
		name: names[index] as string,
		locale: locales[index],
	}));
}

function oneDirectory(files: readonly string[]): void {
	const directories = new Set(files.map(dirname));
	if (directories.size > 1) {
		throw new Error(
			`these files are in ${directories.size} directories, and a set read by what its names share is one directory. Audit them one directory at a time.`,
		);
	}
}

/** One catalogue's text, or the reason it could not be had — never naming the path. */
export async function readText(fs: FileSystem, path: string): Promise<string> {
	const text = utf8(await fs.readFile(path));
	if (text === undefined) throw new Error('not UTF-8 text');
	return text;
}
