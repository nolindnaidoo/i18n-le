import * as vscode from 'vscode';
import type { Finding } from '../audit/audit';
import { describeShape } from '../audit/layout';
import type { Report } from '../audit/report';

/**
 * The report a person reads, as Markdown.
 *
 * **Nothing in it is a translated string.** It carries what the engine carries:
 * key names, placeholder tokens, counts, styles, shapes and byte offsets. Keys
 * are the deliberate exception — a finding that will not name its key is not a
 * finding — and where the layout makes the key the English sentence, that
 * English is shown. A translation never is.
 */
export function formatReport(report: Report, named: boolean): string {
	const lines: string[] = [`# ${vscode.l10n.t('i18n-LE report')}`, ''];
	lines.push(summary(report), '');
	lines.push(
		`> ${vscode.l10n.t('Only key names and structural facts are shown. No translated text appears in this report.')}`,
		'',
	);

	lines.push(`## ${vscode.l10n.t('Why {0}', report.system.library)}`, '');
	if (named)
		lines.push(`- ${vscode.l10n.t('Named in the i18n-le.library setting.')}`);
	for (const signal of report.system.evidence)
		lines.push(`- ${signal.class}: ${code(signal.detail)}`);
	if (!named && report.system.evidence.length === 0)
		lines.push(`- ${vscode.l10n.t('No evidence recorded.')}`);
	lines.push('');

	if (report.files.length > 0) {
		lines.push(`## ${vscode.l10n.t('Catalogues')}`, '');
		for (const file of report.files) {
			const role =
				report.source?.path === file.path
					? ` · ${vscode.l10n.t('source')}`
					: '';
			lines.push(
				`- ${code(file.path)} · ${file.locale ?? vscode.l10n.t('base')} · ${vscode.l10n.t('{0} key(s)', file.keys)}${role}`,
			);
		}
		lines.push('');
	}

	for (const [file, findings] of grouped(report.findings)) {
		lines.push(`## ${code(file)}`, '');
		for (const finding of findings) {
			const evidence = describeEvidence(finding);
			lines.push(
				`- **${code(finding.key)}** · ${finding.kind} · ${finding.severity}${evidence ? ` · ${evidence}` : ''}`,
			);
		}
		lines.push('');
	}

	if (report.diagnostics.length > 0) {
		lines.push(`## ${vscode.l10n.t('Not read')}`, '');
		for (const diagnostic of report.diagnostics) {
			lines.push(
				`- ${code(diagnostic.file)} (${diagnostic.code}): ${diagnostic.message}`,
			);
		}
		lines.push('');
	}
	return `${lines.join('\n')}`;
}

function summary(report: Report): string {
	const layout = report.system.layout
		? describeShape(report.system.layout)
		: '';
	const parts = [`**${report.system.library}**`];
	if (report.system.version) parts.push(report.system.version);
	if (layout) parts.push(code(layout));
	if (report.status === 'no-files') {
		parts.push(vscode.l10n.t('no catalogues found here'));
		return parts.join(' · ');
	}
	parts.push(vscode.l10n.t('{0} catalogue(s)', report.summary.files));
	parts.push(
		report.status === 'clean'
			? vscode.l10n.t('nothing structurally wrong')
			: vscode.l10n.t('{0} finding(s)', report.summary.findings),
	);
	return parts.join(' · ');
}

/** Findings in report order, grouped by file and keeping first-seen file order. */
function grouped(findings: readonly Finding[]): Map<string, Finding[]> {
	const out = new Map<string, Finding[]>();
	for (const finding of findings) {
		const list = out.get(finding.file);
		if (list) list.push(finding);
		else out.set(finding.file, [finding]);
	}
	return out;
}

/** The evidence a finding carries, in words. Every field is metadata. */
export function describeEvidence(finding: Finding): string {
	const f = finding as Record<string, unknown>;
	if (Array.isArray(f.sourceTokens) && Array.isArray(f.targetTokens)) {
		return vscode.l10n.t(
			'source {0}, target {1}',
			tokens(f.sourceTokens),
			tokens(f.targetTokens),
		);
	}
	if (typeof f.sourceCount === 'number' && typeof f.targetCount === 'number') {
		return vscode.l10n.t(
			'{0} placeholder(s) in the source, {1} here',
			f.sourceCount,
			f.targetCount,
		);
	}
	if (typeof f.sourceStyle === 'string' && typeof f.targetStyle === 'string') {
		return vscode.l10n.t(
			'source {0}, target {1}',
			f.sourceStyle,
			f.targetStyle,
		);
	}
	if (typeof f.sourceShape === 'string' && typeof f.targetShape === 'string') {
		return vscode.l10n.t(
			'source {0}, target {1}',
			f.sourceShape,
			f.targetShape,
		);
	}
	if (typeof f.occurrences === 'number')
		return vscode.l10n.t('defined {0} times', f.occurrences);
	if (typeof f.construct === 'string' && typeof f.offset === 'number') {
		return vscode.l10n.t('{0} at byte {1}', f.construct, f.offset);
	}
	return '';
}

function tokens(list: readonly unknown[]): string {
	return list.length === 0
		? '—'
		: list.map((token) => code(String(token))).join(' ');
}

/**
 * Text as a code span. A code span cannot escape a backtick, so one becomes a
 * quote; an empty key is shown as `""`, which is how JSON wrote it.
 */
function code(text: string): string {
	if (text === '') return '`""`';
	return `\`${text.replace(/`/g, "'").replace(/\r?\n/g, ' ')}\``;
}
