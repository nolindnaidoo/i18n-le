import * as vscode from 'vscode';
import type { Telemetry } from '../telemetry/telemetry';
import type { Notifier } from '../ui/notifier';
import type { RatingPrompt } from '../ui/ratingPrompt';
import type { StatusBar } from '../ui/statusBar';
import { auditCatalogues } from './audit';
import { guarded } from './guard';

export interface CommandDependencies {
	notifier: Notifier;
	ratingPrompt: RatingPrompt;
	statusBar: StatusBar;
	telemetry: Telemetry;
}

export function registerCommands(
	context: vscode.ExtensionContext,
	deps: CommandDependencies,
): void {
	context.subscriptions.push(
		// The Explorer passes the folder it was invoked on; the palette passes nothing.
		vscode.commands.registerCommand(
			'i18n-le.audit',
			guarded(deps.notifier, async (picked?: unknown) =>
				auditCatalogues(
					deps,
					picked instanceof vscode.Uri ? picked : undefined,
				),
			),
		),
	);
}
