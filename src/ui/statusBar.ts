import * as vscode from 'vscode';
import { readConfig } from '../config/config';

export interface StatusBar {
	flash(text: string): void;
}

const IDLE = '$(globe) i18n-LE';

export function createStatusBar(context: vscode.ExtensionContext): StatusBar {
	const item = vscode.window.createStatusBarItem(
		vscode.StatusBarAlignment.Left,
		100,
	);
	item.text = IDLE;
	item.tooltip = vscode.l10n.t('Run i18n-LE: Audit Catalogues');
	item.command = 'i18n-le.audit';
	context.subscriptions.push(item);

	function updateVisibility(): void {
		if (readConfig().statusBarEnabled) item.show();
		else item.hide();
	}

	updateVisibility();
	context.subscriptions.push(
		vscode.workspace.onDidChangeConfiguration((e) => {
			if (e.affectsConfiguration('i18n-le.statusBar.enabled'))
				updateVisibility();
		}),
	);

	let hideTimer: NodeJS.Timeout | undefined;
	context.subscriptions.push({
		dispose(): void {
			if (hideTimer) clearTimeout(hideTimer);
			hideTimer = undefined;
		},
	});

	return Object.freeze({
		flash(text: string): void {
			// A short-lived status text instead of a notification.
			if (!readConfig().statusBarEnabled) return;
			item.text = `$(eye) ${text}`;
			if (hideTimer) clearTimeout(hideTimer);
			hideTimer = setTimeout(() => {
				item.text = IDLE;
				hideTimer = undefined;
			}, 2000);
		},
	});
}
