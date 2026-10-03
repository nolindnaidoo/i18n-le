import * as assert from 'node:assert';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import * as vscode from 'vscode';

const EXTENSION_ID = 'nolindnaidoo.i18n-le';

/** A small i18next project on disk, so identification has a manifest and syntax to read. */
function project(): string {
	const dir = mkdtempSync(join(tmpdir(), 'i18n-le-it-'));
	writeFileSync(join(dir, 'package.json'), JSON.stringify({ dependencies: { i18next: '^26.0.0' } }));
	mkdirSync(join(dir, 'locales'));
	writeFileSync(join(dir, 'locales', 'en.json'), JSON.stringify({ nav: { home: 'Home' }, greeting: 'Hello {{name}}' }));
	writeFileSync(join(dir, 'locales', 'de.json'), JSON.stringify({ greeting: 'Hallo {{vorname}}' }));
	return dir;
}

describe('i18n-LE integration', function () {
	this.timeout(30_000);

	it('activates', async () => {
		const extension = vscode.extensions.getExtension(EXTENSION_ID);
		assert.ok(extension, `extension ${EXTENSION_ID} not found`);
		await extension.activate();
		assert.strictEqual(extension.isActive, true);
	});

	it('registers every declared command', async () => {
		const extension = vscode.extensions.getExtension(EXTENSION_ID);
		await extension?.activate();
		const commands = await vscode.commands.getCommands(true);
		for (const id of ['i18n-le.audit', 'i18n-le.openSettings', 'i18n-le.help']) {
			assert.ok(commands.includes(id), `missing command: ${id}`);
		}
	});

	it('identifies the library and audits the set the open catalogue belongs to, without quoting it', async () => {
		const dir = project();
		const document = await vscode.workspace.openTextDocument(vscode.Uri.file(join(dir, 'locales', 'de.json')));
		await vscode.window.showTextDocument(document);

		await vscode.commands.executeCommand('i18n-le.audit');

		const report = vscode.workspace.textDocuments.find(
			(doc) => doc.languageId === 'markdown' && doc.getText().includes('i18n-LE report'),
		);
		assert.ok(report, 'no report document found');
		const text = report.getText();
		assert.ok(text.includes('**i18next**'), 'the library was not identified');
		assert.ok(text.includes('`nav.home`') && text.includes('missing-key'));
		assert.ok(text.includes('source `name`, target `vorname`'));
		assert.ok(!text.includes('Hallo'), 'the report carried a translation');
	});

	it('audits a folder picked in the Explorer', async () => {
		const dir = project();
		await vscode.commands.executeCommand('i18n-le.audit', vscode.Uri.file(join(dir, 'locales')));
		const report = vscode.workspace.textDocuments.find(
			(doc) => doc.languageId === 'markdown' && doc.getText().includes('placeholder-name-mismatch'),
		);
		assert.ok(report, 'no report for the picked folder');
	});

	it('offers its MCP server to agent mode', async () => {
		// The registration itself is only observable in a real host, which
		// scripts/e2e-vsix.js covers against the installed VSIX.
		const extension = vscode.extensions.getExtension(EXTENSION_ID);
		await extension?.activate();
		assert.strictEqual(
			typeof vscode.lm.registerMcpServerDefinitionProvider,
			'function',
			'this VS Code build predates the MCP provider API',
		);
		const providers = extension?.packageJSON.contributes.mcpServerDefinitionProviders as {
			id: string;
			label: string;
		}[];
		assert.deepStrictEqual(
			providers.map((p) => p.id),
			['i18n-le'],
		);
	});
});
