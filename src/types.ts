import type { LibraryId } from './audit/library';

export type NotificationLevel = 'all' | 'important' | 'silent';

/** `auto` identifies the library from the project; a name skips identification. */
export type LibrarySetting = 'auto' | LibraryId;

/** The extension's settings, read once per command and frozen. */
export interface Configuration {
	readonly copyToClipboardEnabled: boolean;
	readonly keysAreSource: boolean;
	readonly library: LibrarySetting;
	readonly notificationsLevel: NotificationLevel;
	readonly openResultsSideBySide: boolean;
	/** A path or a language tag; empty means auto-detect the English one. */
	readonly source: string;
	readonly statusBarEnabled: boolean;
	readonly telemetryEnabled: boolean;
}
