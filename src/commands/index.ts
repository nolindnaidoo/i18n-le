import * as vscode from 'vscode';
import type { Telemetry } from '../telemetry/telemetry';
import type { Notifier } from '../ui/notifier';
import type { StatusBar } from '../ui/statusBar';
import { auditCatalogues } from './audit';

export interface CommandDependencies {
	notifier: Notifier;
	statusBar: StatusBar;
	telemetry: Telemetry;
}

export function registerCommands(
	context: vscode.ExtensionContext,
	deps: CommandDependencies,
): void {
	context.subscriptions.push(
		// The Explorer passes the folder it was invoked on; the palette passes nothing.
		vscode.commands.registerCommand('i18n-le.audit', async (picked?: unknown) =>
			auditCatalogues(deps, picked instanceof vscode.Uri ? picked : undefined),
		),
	);
}
