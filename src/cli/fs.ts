/**
 * The engine's filesystem seam over `node:fs`, as the identification port
 * check reads a tree: an unreadable directory is empty, as
 * `read_dir(...).into_iter().flatten()` reads it, and a path that cannot be
 * stat'd is not there.
 */
import { readdirSync, readFileSync, realpathSync, statSync } from 'node:fs';
import { dirname, resolve, sep } from 'node:path';
import type { EntryType, FileSystem } from '../audit/fs';
import { rustIoMessage } from './ioError';

function typeOf(entry: {
	isDirectory(): boolean;
	isFile(): boolean;
}): EntryType {
	if (entry.isDirectory()) return 'directory';
	return entry.isFile() ? 'file' : 'other';
}

export const nodeFs: FileSystem = Object.freeze({
	async readDirectory(path: string) {
		try {
			return readdirSync(path, { withFileTypes: true }).map(
				(entry): readonly [string, EntryType] => [entry.name, typeOf(entry)],
			);
		} catch {
			return [];
		}
	},
	async stat(path: string) {
		try {
			const stats = statSync(path);
			return { type: typeOf(stats), size: stats.size };
		} catch {
			return undefined;
		}
	},
	// The reason a catalogue was skipped is Rust's `io::Error`, as the
	// crate's `read_text` reports it.
	async readFile(path: string) {
		try {
			return readFileSync(path);
		} catch (error) {
			throw new Error(rustIoMessage(error));
		}
	},
});

/**
 * The engine reads absolute, `/`-separated paths, and the crate anchors on
 * the canonical one: on macOS `/tmp` is a link to `/private/tmp`, and the
 * two would walk different ancestors looking for a manifest.
 */
export function enginePath(input: string): string {
	let absolute: string;
	try {
		absolute = realpathSync(input);
	} catch {
		absolute = resolve(input);
	}
	return sep === '/' ? absolute : absolute.split(sep).join('/');
}

/**
 * The crate's `anchor_of`: the first input must exist, and the directory it
 * names or sits in must canonicalize. Each refusal names the path as typed.
 */
export function checkAnchor(first: string): void {
	let isDirectory: boolean;
	try {
		isDirectory = statSync(first).isDirectory();
	} catch (error) {
		throw new Error(`${first}: ${rustIoMessage(error)}`);
	}
	const parent = dirname(first);
	const directory = isDirectory ? first : parent === '' ? '.' : parent;
	try {
		realpathSync(directory);
	} catch (error) {
		throw new Error(`${directory}: ${rustIoMessage(error)}`);
	}
}
