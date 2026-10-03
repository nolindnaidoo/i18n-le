import * as vscode from 'vscode';
import { parseLibrary } from '../audit/library';
import type {
	Configuration,
	LibrarySetting,
	NotificationLevel,
} from '../types';

/**
 * The defaults, exported for the parity gate: `config.test.ts` asserts they
 * match every default declared in package.json, which is what stops the two
 * drifting apart.
 */
export const CONFIG_DEFAULTS = Object.freeze({
	copyToClipboardEnabled: false,
	keysAreSource: false,
	library: 'auto' as const,
	notificationsLevel: 'silent' as const,
	openResultsSideBySide: true,
	source: '',
	statusBarEnabled: true,
	telemetryEnabled: false,
});

export function readConfig(): Configuration {
	const config = vscode.workspace.getConfiguration('i18n-le');
	return Object.freeze({
		copyToClipboardEnabled: readBoolean(
			config,
			'copyToClipboardEnabled',
			CONFIG_DEFAULTS.copyToClipboardEnabled,
		),
		keysAreSource: readBoolean(
			config,
			'keysAreSource',
			CONFIG_DEFAULTS.keysAreSource,
		),
		library: readLibrary(config),
		notificationsLevel: readNotificationLevel(config),
		openResultsSideBySide: readBoolean(
			config,
			'openResultsSideBySide',
			CONFIG_DEFAULTS.openResultsSideBySide,
		),
		source: readString(config, 'source', CONFIG_DEFAULTS.source),
		statusBarEnabled: readBoolean(
			config,
			'statusBar.enabled',
			CONFIG_DEFAULTS.statusBarEnabled,
		),
		telemetryEnabled: readBoolean(
			config,
			'telemetryEnabled',
			CONFIG_DEFAULTS.telemetryEnabled,
		),
	});
}

function readBoolean(
	config: vscode.WorkspaceConfiguration,
	key: string,
	defaultValue: boolean,
): boolean {
	const value = config.get(key, defaultValue);
	return typeof value === 'boolean' ? value : defaultValue;
}

function readString(
	config: vscode.WorkspaceConfiguration,
	key: string,
	defaultValue: string,
): string {
	const value = config.get(key, defaultValue);
	return typeof value === 'string' ? value.trim() : defaultValue;
}

/** An unknown name falls back to `auto`: identification refuses rather than guesses. */
function readLibrary(config: vscode.WorkspaceConfiguration): LibrarySetting {
	const raw = config.get<unknown>('library', CONFIG_DEFAULTS.library);
	if (raw === 'auto' || typeof raw !== 'string') return CONFIG_DEFAULTS.library;
	return parseLibrary(raw) ?? CONFIG_DEFAULTS.library;
}

export function isValidNotificationLevel(v: unknown): v is NotificationLevel {
	return v === 'all' || v === 'important' || v === 'silent';
}

function readNotificationLevel(
	config: vscode.WorkspaceConfiguration,
): NotificationLevel {
	const raw = config.get<string>(
		'notificationsLevel',
		CONFIG_DEFAULTS.notificationsLevel,
	);
	return isValidNotificationLevel(raw)
		? raw
		: CONFIG_DEFAULTS.notificationsLevel;
}
