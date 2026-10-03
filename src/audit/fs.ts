/**
 * What identification and layout need from a filesystem, and nothing more.
 *
 * The engine reads through this rather than through `vscode.workspace.fs` or
 * `node:fs`, so it runs unchanged in the extension, in a test against an
 * in-memory tree, and in the script that holds it against the crate's CLI.
 * Paths are POSIX strings: absolute, `/`-separated, never ending in `/` except
 * for the root.
 */
export type EntryType = 'file' | 'directory' | 'other';

export interface FileSystem {
	/** The entries directly inside a directory, or `[]` when it cannot be read. */
	readDirectory(
		path: string,
	): Promise<ReadonlyArray<readonly [string, EntryType]>>;
	/** What a path is and how big, or undefined when there is nothing there. */
	stat(
		path: string,
	): Promise<{ readonly type: EntryType; readonly size: number } | undefined>;
	/** A file's bytes; throws with the reason when they cannot be had. */
	readFile(path: string): Promise<Uint8Array>;
}

export function join(directory: string, name: string): string {
	return directory === '/' ? `/${name}` : `${directory}/${name}`;
}

export function dirname(path: string): string {
	const at = path.lastIndexOf('/');
	if (at <= 0) return '/';
	return path.slice(0, at);
}

export function basename(path: string): string {
	return path.slice(path.lastIndexOf('/') + 1);
}

/** `Path::ancestors().take(n)`: the path itself, then each parent, `n` in all. */
export function ancestors(path: string, n: number): string[] {
	const out: string[] = [];
	let current = path;
	while (out.length < n) {
		out.push(current);
		if (current === '/') break;
		current = dirname(current);
	}
	return out;
}

/** The extension after the last dot of the name, as `Path::extension` reads it. */
export function extensionOf(path: string): string | undefined {
	const name = basename(path);
	const at = name.lastIndexOf('.');
	if (at <= 0) return undefined;
	return name.slice(at + 1);
}

/** Plain code-unit order, never locale order: a report must not move with the UI language. */
export function byPath(a: string, b: string): number {
	return a < b ? -1 : Number(a > b);
}

const strict = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

/** Bytes as UTF-8 text, or undefined when they are not. */
export function utf8(bytes: Uint8Array): string | undefined {
	try {
		return strict.decode(bytes);
	} catch {
		// `fatal` is the only way TextDecoder reports invalid UTF-8; the caller
		// turns undefined into a named reason rather than a lossy decode.
		return undefined;
	}
}
