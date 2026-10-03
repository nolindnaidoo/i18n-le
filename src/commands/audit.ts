import * as vscode from 'vscode';
import { canonicalise } from '../audit/locale';
import { scan } from '../audit/scan';
import { readConfig } from '../config/config';
import { formatReport } from '../report/format';
import { errorMessage } from '../utils/errors';
import type { CommandDependencies } from './index';
import { showReport } from './output';
import { workspaceFileSystem } from './workspaceFs';

/** How a refusal tells the reader to name the library, in this surface's words. */
export const HOW_TO_NAME = 'in the i18n-le.library setting';

/**
 * Audit the catalogue set a folder holds: the folder picked in the Explorer, or
 * the one holding the active document.
 *
 * A folder named for a locale — `locales/de` — is one locale of a namespaced
 * set, so its parent is what gets audited. Everything after that is the
 * engine's: identify the library, read the set, audit it, or refuse by name.
 */
export async function auditCatalogues(
	deps: CommandDependencies,
	picked?: vscode.Uri,
): Promise<void> {
	deps.telemetry.event('command', { name: 'audit' });
	const folder = await targetFolder(picked);
	if (!folder) {
		deps.notifier.error(
			vscode.l10n.t('Open a catalogue, or pick its folder in the Explorer'),
		);
		return;
	}

	const config = readConfig();
	const named = config.library === 'auto' ? undefined : config.library;
	let report: Awaited<ReturnType<typeof scan>>;
	try {
		report = await scan({
			fs: workspaceFileSystem(folder),
			inputs: [folder.path],
			named,
			source: config.source === '' ? undefined : config.source,
			keysAreSource: config.keysAreSource,
			howToName: HOW_TO_NAME,
		});
	} catch (error) {
		// A refusal is the answer to a malformed question, not a crash: it names
		// what was found and what would settle it.
		deps.notifier.error(
			vscode.l10n.t(
				'i18n-LE could not audit this folder: {0}',
				errorMessage(error),
			),
		);
		deps.telemetry.event('refused', {});
		return;
	}

	await showReport(formatReport(report, named !== undefined), config, deps);
	deps.telemetry.event('audited', {
		files: String(report.summary.files),
		findings: String(report.summary.findings),
	});
	deps.statusBar.flash(
		vscode.l10n.t('{0} finding(s)', report.summary.findings),
	);
	if (report.summary.findings > 0) {
		deps.notifier.warn(
			vscode.l10n.t(
				'Found {0} catalogue problem(s) in {1} file(s)',
				report.summary.findings,
				report.summary.files,
			),
		);
	}
}

async function targetFolder(
	picked: vscode.Uri | undefined,
): Promise<vscode.Uri | undefined> {
	const start = picked ?? vscode.window.activeTextEditor?.document.uri;
	if (!start || start.scheme === 'untitled') return undefined;
	let folder = start;
	try {
		const stat = await vscode.workspace.fs.stat(start);
		if (!(stat.type & vscode.FileType.Directory))
			folder = vscode.Uri.joinPath(start, '..');
	} catch {
		return undefined;
	}
	const name = folder.path.slice(folder.path.lastIndexOf('/') + 1);
	return canonicalise(name) === undefined
		? folder
		: vscode.Uri.joinPath(folder, '..');
}
