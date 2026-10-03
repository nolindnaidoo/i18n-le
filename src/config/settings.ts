import * as vscode from 'vscode';
import type { Telemetry } from '../telemetry/telemetry';

// Command to navigate users directly to i18n-le settings
export function registerOpenSettingsCommand(
	context: vscode.ExtensionContext,
	telemetry: Telemetry,
): void {
	context.subscriptions.push(
		vscode.commands.registerCommand('i18n-le.openSettings', async () => {
			telemetry.event('command', { name: 'openSettings' });
			// Open Settings UI filtered by exact setting prefix to avoid unrelated matches
			await vscode.commands.executeCommand(
				'workbench.action.openSettings',
				'i18n-le.',
			);
		}),
	);
}
