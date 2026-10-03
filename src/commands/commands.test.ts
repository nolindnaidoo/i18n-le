import { beforeEach, describe, expect, it } from 'vitest';
import {
	_clipboardText,
	_createDocument,
	_createExtensionContext,
	_openedDocuments,
	_registeredCommands,
	_resetMockState,
	_setActiveEditor,
	_setConfig,
	_setWorkspaceFiles,
	_shownMessages,
	executedBuiltins,
	Uri,
} from '../__mocks__/vscode';
import { registerOpenSettingsCommand } from '../config/settings';
import type { Telemetry } from '../telemetry/telemetry';
import { createNotifier } from '../ui/notifier';
import type { StatusBar } from '../ui/statusBar';
import { generateHelpContent, registerHelpCommand } from './help';
import { registerCommands } from './index';

function makeDeps() {
	const events: string[] = [];
	const telemetry: Telemetry = {
		event: (name, properties) =>
			events.push(properties ? `${name}:${JSON.stringify(properties)}` : name),
		dispose: () => {},
	};
	const statusBar: StatusBar = { flash: () => {} };
	return { deps: { notifier: createNotifier(), statusBar, telemetry }, events };
}

async function runCommand(id: string, ...args: unknown[]): Promise<void> {
	const handler = _registeredCommands().get(id);
	if (!handler) throw new Error(`command not registered: ${id}`);
	await handler(...args);
}

function report(): string {
	const last = _openedDocuments().at(-1);
	if (!last) throw new Error('no report was opened');
	return last.getText();
}

/** An i18next project as it sits on disk: a manifest, a call site, two locales. */
const I18NEXT_PROJECT = {
	'/app/package.json': '{"dependencies":{"i18next":"^26.2.0"}}',
	'/app/src/app.ts': 'const { t } = useTranslation();\n',
	'/app/locales/en.json':
		'{"greeting":"Hello {{name}}","bye":"Goodbye","same":"OK"}',
	'/app/locales/es.json':
		'{"greeting":"Hola {{nombre}}","same":"OK","extra":"Sobrante"}',
};

beforeEach(() => {
	_resetMockState();
	const { deps } = makeDeps();
	registerCommands(_createExtensionContext() as never, deps);
});

describe('i18n-le.audit', () => {
	it('asks for a catalogue when nothing is open and nothing was picked', async () => {
		await runCommand('i18n-le.audit');
		expect(_shownMessages()[0]).toMatchObject({
			kind: 'error',
			message: expect.stringMatching(/Open a catalogue/),
		});
	});

	it('identifies the library from the project and reports what is wrong, by key', async () => {
		_setWorkspaceFiles(I18NEXT_PROJECT);
		_setActiveEditor(
			_createDocument({ content: '', fileName: '/app/locales/es.json' }),
		);
		await runCommand('i18n-le.audit');
		const text = report();
		expect(text).toContain('**i18next** · ^26.2.0');
		expect(text).toContain('- manifest: `i18next in ../package.json`');
		expect(text).toContain('- call-site: `useTranslation( in ../src/app.ts`');
		expect(text).toContain(
			'- **`greeting`** · placeholder-name-mismatch · error · source `name`, target `nombre`',
		);
		expect(text).toContain('- **`bye`** · missing-key · error');
		expect(text).toContain('- **`extra`** · extra-key · error');
		expect(text).toContain('- **`same`** · untranslated · info');
	});

	it('never puts a translated string in the report', async () => {
		_setWorkspaceFiles(I18NEXT_PROJECT);
		_setActiveEditor(
			_createDocument({ content: '', fileName: '/app/locales/es.json' }),
		);
		await runCommand('i18n-le.audit');
		for (const translated of ['Hola', 'Sobrante', 'Hello', 'Goodbye'])
			expect(report()).not.toContain(translated);
	});

	it('audits the folder picked in the Explorer', async () => {
		_setWorkspaceFiles(I18NEXT_PROJECT);
		await runCommand('i18n-le.audit', Uri.file('/app/locales'));
		expect(report()).toContain('placeholder-name-mismatch');
	});

	it('audits a namespaced set from inside one of its locale folders', async () => {
		_setWorkspaceFiles({
			'/app/package.json': '{"dependencies":{"i18next":"^26.0.0"}}',
			'/app/locales/en/common.json': '{"a":"{{x}}"}',
			'/app/locales/de/common.json': '{"a":"{{y}}"}',
		});
		_setActiveEditor(
			_createDocument({ content: '', fileName: '/app/locales/de/common.json' }),
		);
		await runCommand('i18n-le.audit');
		expect(report()).toContain('`<locale>/<namespace>.json`');
		expect(report()).toContain('## `de/common.json`');
	});

	it('refuses to guess when nothing identifies a library, and says how to name one', async () => {
		_setWorkspaceFiles({
			'/x/en.json': '{"a":"b"}',
			'/x/de.json': '{"a":"c"}',
		});
		await runCommand('i18n-le.audit', Uri.file('/x'));
		expect(_openedDocuments()).toHaveLength(0);
		expect(_shownMessages()[0]).toMatchObject({
			kind: 'error',
			message: expect.stringMatching(
				/no i18n library could be identified.*Name it in the i18n-le\.library setting\./,
			),
		});
	});

	it('takes the library from the setting, which skips identification', async () => {
		_setConfig('i18n-le.library', 'next-intl');
		_setWorkspaceFiles({
			'/x/en.json': '{"a":"{n}"}',
			'/x/de.json': '{"a":"{m}"}',
		});
		await runCommand('i18n-le.audit', Uri.file('/x'));
		expect(report()).toContain('Named in the i18n-le.library setting.');
		expect(report()).toContain('placeholder-name-mismatch');
	});

	it('uses the source the setting names', async () => {
		_setConfig('i18n-le.library', 'i18next');
		_setConfig('i18n-le.source', 'de');
		_setWorkspaceFiles({
			'/x/en.json': '{"a":"b","c":"d"}',
			'/x/de.json': '{"a":"e"}',
		});
		await runCommand('i18n-le.audit', Uri.file('/x'));
		expect(report()).toContain('`de.json` · de · 1 key(s) · source');
		expect(report()).toContain('- **`c`** · extra-key · error');
	});

	it('names a catalogue that is not UTF-8 and still audits the rest', async () => {
		_setConfig('i18n-le.library', 'i18next');
		_setWorkspaceFiles({
			'/x/en.json': '{"a":"b"}',
			'/x/de.json': new Uint8Array([0x7b, 0xff, 0x7d]),
		});
		await runCommand('i18n-le.audit', Uri.file('/x'));
		expect(report()).toContain('- `de.json` (skipped): not UTF-8 text');
	});

	it('copies the report when asked to', async () => {
		_setConfig('i18n-le.copyToClipboardEnabled', true);
		_setWorkspaceFiles(I18NEXT_PROJECT);
		await runCommand('i18n-le.audit', Uri.file('/app/locales'));
		expect(_clipboardText()).toBe(report());
	});
});

describe('settings and help', () => {
	it('opens the settings filtered to this extension', async () => {
		const { deps } = makeDeps();
		registerOpenSettingsCommand(
			_createExtensionContext() as never,
			deps.telemetry,
		);
		await runCommand('i18n-le.openSettings');
		expect(executedBuiltins.at(-1)).toMatchObject({ args: ['i18n-le.'] });
	});

	it('opens the help, which names every check and every library', async () => {
		const { deps } = makeDeps();
		registerHelpCommand(_createExtensionContext() as never, deps.telemetry);
		await runCommand('i18n-le.help');
		const help = generateHelpContent();
		for (const kind of [
			'missing-key',
			'extra-key',
			'placeholder-count-mismatch',
			'placeholder-name-mismatch',
			'placeholder-style-mismatch',
			'convention-mismatch',
			'duplicate-key-within-file',
			'structure-mismatch',
			'empty-value',
			'untranslated',
			'i18next',
			'next-intl',
			'vscode-l10n',
			'flutter-arb',
		]) {
			expect(help).toContain(kind);
		}
	});
});
