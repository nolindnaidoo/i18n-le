import * as vscode from 'vscode';
import type { Telemetry } from '../telemetry/telemetry';

/**
 * Register the help command
 */
export function registerHelpCommand(
	context: vscode.ExtensionContext,
	telemetry: Telemetry,
): void {
	const disposable = vscode.commands.registerCommand(
		'i18n-le.help',
		async () => {
			telemetry.event('command', { name: 'help' });
			await showHelp();
		},
	);
	context.subscriptions.push(disposable);
}

async function showHelp(): Promise<void> {
	const doc = await vscode.workspace.openTextDocument({
		content: generateHelpContent(),
		language: 'markdown',
	});
	await vscode.window.showTextDocument(doc, {
		preview: false,
		viewColumn: vscode.ViewColumn.Beside,
	});
}

/** Every claim here is the crate's behaviour, and the corpus pins it. */
export function generateHelpContent(): string {
	return [
		'# i18n-LE Help',
		'',
		'Audits a set of translation catalogues against one of them and reports what is structurally wrong. Only key names and structural facts are shown — never a translated string.',
		'',
		'## Commands',
		'',
		'- **Audit Catalogues**: the set the active catalogue belongs to. From the Explorer, the folder it is invoked on. A folder named for a locale, such as `locales/de`, is one locale of a namespaced set, so its parent is audited.',
		'',
		'## Which library',
		'',
		"The library is identified before anything is read, from five kinds of evidence: a manifest dependency (`package.json`, `pubspec.yaml`), a config file, the directory layout, the catalogue syntax, and call sites in source. Two agreeing is an identification; an ARB file's `@@locale` beside `@key` metadata settles it alone. When nothing agrees, or two libraries do, the audit refuses and names what it found. Set `i18n-le.library` to skip identification.",
		'',
		'| Library | Placeholders | Plurals |',
		'|---|---|---|',
		'| `i18next` | `{{name}}`, `$t(key)` nesting | `key_one`, `key_other` suffixes |',
		'| `next-intl` | ICU `{name}`, `{count, plural, ...}` | inside the message |',
		'| `vscode-l10n` | `{0}` | none |',
		'| `flutter-arb` | ICU, with `@key` metadata | inside the message |',
		'',
		'## The checks',
		'',
		'| Kind | Severity | What |',
		'|---|---|---|',
		'| `missing-key` | error | In the source, absent from the target. |',
		'| `extra-key` | error | In the target, absent from the source. |',
		'| `placeholder-count-mismatch` | error | A placeholder was dropped or added. |',
		'| `placeholder-name-mismatch` | error | Same count, different names. |',
		'| `placeholder-style-mismatch` | error | A placeholder written in a style this library does not read. |',
		'| `convention-mismatch` | error | A construct from another convention, rendered verbatim at runtime. |',
		'| `duplicate-key-within-file` | error | A key defined twice; every loader keeps the last. |',
		'| `structure-mismatch` | error | An object in one locale and a string in another. |',
		'| `empty-value` | warning | Empty or only whitespace. |',
		'| `untranslated` | info | Byte-identical to the source. |',
		'',
		'## The source',
		'',
		'One catalogue is the contract, never the union of all of them. It is the one English catalogue — tagged `en`, or untagged as `bundle.l10n.json` and `package.nls.json` are. When there is none, or several, set `i18n-le.source` to a file name or a language tag.',
		'',
		'## Agents',
		'',
		"The bundled MCP server offers `check_catalogues` to agent mode. It answers exactly as the `i18n-le` command-line tool's server does, and takes the library as an argument because it cannot see the project.",
		'',
		'## Troubleshooting',
		'',
		'- **"no i18n library could be identified"**: set `i18n-le.library`.',
		'- **"catalogues could be the English one"**: set `i18n-le.source`.',
		'- **A file under "Not read"**: it is not UTF-8, or not a JSON object; the reason is beside it, and the rest were still audited.',
		'',
	].join('\n');
}
