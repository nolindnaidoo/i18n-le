import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { _resetMockState, _setConfig } from '../__mocks__/vscode';
import { libraryNames } from '../audit/library';
import {
	CONFIG_DEFAULTS,
	isValidNotificationLevel,
	readConfig,
} from './config';

/**
 * CONFIG_DEFAULTS must stay identical to the defaults declared in package.json
 * contributes.configuration; a sibling shipped with the two silently
 * disagreeing.
 */
describe('config defaults parity with package.json', () => {
	const manifest = JSON.parse(
		readFileSync(join(__dirname, '..', '..', 'package.json'), 'utf8'),
	) as {
		contributes: {
			configuration: {
				properties: Record<string, { default: unknown; enum?: string[] }>;
			};
		};
	};
	const props = manifest.contributes.configuration.properties;

	const KEY_MAP: Record<string, keyof typeof CONFIG_DEFAULTS> = {
		'i18n-le.copyToClipboardEnabled': 'copyToClipboardEnabled',
		'i18n-le.keysAreSource': 'keysAreSource',
		'i18n-le.library': 'library',
		'i18n-le.notificationsLevel': 'notificationsLevel',
		'i18n-le.openResultsSideBySide': 'openResultsSideBySide',
		'i18n-le.source': 'source',
		'i18n-le.statusBar.enabled': 'statusBarEnabled',
		'i18n-le.telemetryEnabled': 'telemetryEnabled',
	};

	it('covers every declared setting', () => {
		expect(Object.keys(props).sort()).toEqual(Object.keys(KEY_MAP).sort());
	});

	for (const [manifestKey, defaultsKey] of Object.entries(KEY_MAP)) {
		it(`${manifestKey} default matches`, () => {
			expect(CONFIG_DEFAULTS[defaultsKey]).toEqual(props[manifestKey]?.default);
		});
	}

	it('offers exactly the libraries the engine reads, after auto', () => {
		expect(props['i18n-le.library']?.enum).toEqual(['auto', ...libraryNames()]);
	});
});

describe('readConfig', () => {
	afterEach(() => _resetMockState());

	it('falls back to auto for a library the engine does not read, so identification still refuses rather than guesses', () => {
		_setConfig('i18n-le.library', 'lingui');
		expect(readConfig().library).toBe('auto');
		_setConfig('i18n-le.library', 'next-intl');
		expect(readConfig().library).toBe('next-intl');
	});

	it('falls back to the default for a value of the wrong type', () => {
		_setConfig('i18n-le.keysAreSource', 'yes');
		_setConfig('i18n-le.source', 42);
		expect(readConfig().keysAreSource).toBe(false);
		expect(readConfig().source).toBe('');
	});

	it('trims the source, so a stray space does not name a catalogue that is not there', () => {
		_setConfig('i18n-le.source', ' en ');
		expect(readConfig().source).toBe('en');
	});
});

describe('isValidNotificationLevel', () => {
	it('accepts the three declared levels and nothing else', () => {
		for (const level of ['all', 'important', 'silent'])
			expect(isValidNotificationLevel(level)).toBe(true);
		expect(isValidNotificationLevel('verbose')).toBe(false);
		expect(isValidNotificationLevel(null)).toBe(false);
	});
});
