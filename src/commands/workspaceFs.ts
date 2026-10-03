import * as vscode from 'vscode';
import type { EntryType, FileSystem } from '../audit/fs';

/**
 * The engine's filesystem over `vscode.workspace.fs`, so an audit reads the
 * same files in a local folder, a remote one and a container alike.
 *
 * The engine speaks POSIX paths; every Uri it touches shares the scheme and
 * authority of the folder the audit started from.
 */
export function workspaceFileSystem(anchor: vscode.Uri): FileSystem {
	const at = (path: string): vscode.Uri => anchor.with({ path });
	return Object.freeze({
		async readDirectory(path: string) {
			try {
				const entries = await vscode.workspace.fs.readDirectory(at(path));
				return entries.map(([name, type]): readonly [string, EntryType] => [
					name,
					kindOf(type),
				]);
			} catch {
				// A directory that cannot be listed holds nothing this can read.
				return [];
			}
		},
		async stat(path: string) {
			try {
				const stat = await vscode.workspace.fs.stat(at(path));
				return { type: kindOf(stat.type), size: stat.size };
			} catch {
				// Nothing there.
				return undefined;
			}
		},
		async readFile(path: string) {
			return vscode.workspace.fs.readFile(at(path));
		},
	});
}

/** A symlink resolves to what it points at; anything else is `other`. */
function kindOf(type: vscode.FileType): EntryType {
	if (type & vscode.FileType.Directory) return 'directory';
	if (type & vscode.FileType.File) return 'file';
	return 'other';
}
