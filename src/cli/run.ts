/**
 * The terminal surface, as the Rust CLI's `cli.rs` runs it.
 *
 * stdout is protocol: one JSON report whenever something is reading it, and
 * nothing when it is a terminal. stderr is the summary, and like the report
 * it names keys and structural facts, never a translated string.
 */
import type { Finding } from '../audit/audit';
import type { Report } from '../audit/report';
import { scan } from '../audit/scan';
import { type Cli, parse } from './args';
import { checkAnchor, enginePath, nodeFs } from './fs';
import { USAGE } from './usage';

/** How a refusal tells the reader to name the library, in the crate's words. */
export const HOW_TO_NAME = 'with --system <name>';

export interface RunIo {
	readonly stdout: (text: string) => void;
	readonly stderr: (text: string) => void;
	readonly stdoutIsTerminal: boolean;
	readonly version: string;
}

export async function run(args: readonly string[], io: RunIo): Promise<number> {
	const first = args[0];
	if (first === '--help' || first === '-h') {
		io.stdout(`${USAGE}\n`);
		return 0;
	}
	if (first === '--version' || first === '-V') {
		io.stdout(`i18n-le ${io.version}\n`);
		return 0;
	}
	let cli: Cli;
	let report: Report;
	try {
		cli = parse(args);
		checkAnchor(cli.inputs[0] as string);
		report = await scan({
			fs: nodeFs,
			inputs: cli.inputs.map(enginePath),
			named: cli.named,
			source: cli.source,
			keysAreSource: cli.keysAreSource,
			howToName: HOW_TO_NAME,
		});
	} catch (error) {
		io.stderr(
			`i18n-le: ${error instanceof Error ? error.message : String(error)}\n`,
		);
		return 2;
	}
	if (!io.stdoutIsTerminal) io.stdout(`${JSON.stringify(report)}\n`);
	io.stderr(summary(report));
	return exitCode(report, cli);
}

/** 2 under `--strict` with an unread catalogue, 1 on a failing finding, else 0. */
export function exitCode(report: Report, cli: Cli): number {
	if (cli.strict && report.diagnostics.length > 0) return 2;
	const fails = (finding: Finding) =>
		cli.failOn !== 'findings' || finding.severity !== 'info';
	return report.findings.some(fails) ? 1 : 0;
}

export function summary(report: Report): string {
	const version =
		report.system.version === null ? '' : ` ${report.system.version}`;
	const evidence =
		report.system.evidence.length === 0
			? 'as asked for'
			: report.system.evidence.map((signal) => signal.detail).join(', ');
	const lines = [`${report.system.library}${version} — ${evidence}`];
	for (const diagnostic of report.diagnostics)
		lines.push(`${diagnostic.file}: ${diagnostic.message}`);
	for (const finding of report.findings) lines.push(describe(finding));
	lines.push(tally(report));
	return `${lines.join('\n')}\n`;
}

function tally(report: Report): string {
	const catalogues = plural(report.summary.files, 'catalogue', 'catalogues');
	if (report.status === 'no-files') return 'no catalogues found';
	if (report.status === 'clean') return `clean — ${catalogues}`;
	return `${plural(report.summary.findings, 'finding', 'findings')} across ${catalogues}`;
}

const KINDS: Readonly<Record<Finding['kind'], string>> = Object.freeze({
	'missing-key': 'missing',
	'extra-key': 'extra',
	'placeholder-count-mismatch': 'placeholder count',
	'placeholder-name-mismatch': 'placeholder renamed',
	'placeholder-style-mismatch': 'placeholder style',
	'convention-mismatch': 'wrong convention in',
	'empty-value': 'empty',
	untranslated: 'untranslated',
	'duplicate-key-within-file': 'duplicate',
	'structure-mismatch': 'structure',
});

const STYLES: Readonly<Record<string, string>> = Object.freeze({
	'double-brace': '{{name}}',
	'single-brace': '{name}',
	positional: '{0}',
});

const SHAPES: Readonly<Record<string, string>> = Object.freeze({
	text: 'a string',
	object: 'an object',
	other: 'neither a string nor an object',
});

/** The one-line projection of a finding: a key and a structural fact. */
export function describe(finding: Finding): string {
	const head = `${finding.file}: ${KINDS[finding.kind]} ${finding.key}`;
	const evidence = finding as Finding & Record<string, unknown>;
	if (Array.isArray(evidence.sourceTokens)) {
		return `${head} (${(evidence.sourceTokens as string[]).join(', ')} became ${(evidence.targetTokens as string[]).join(', ')})`;
	}
	if (typeof evidence.sourceCount === 'number') {
		return `${head} (${plural(evidence.sourceCount, 'placeholder', 'placeholders')} became ${evidence.targetCount})`;
	}
	if (typeof evidence.sourceStyle === 'string') {
		return `${head} (${STYLES[evidence.sourceStyle]} became ${STYLES[evidence.targetStyle as string]})`;
	}
	if (typeof evidence.sourceShape === 'string') {
		return `${head} (${SHAPES[evidence.targetShape as string]} here, ${SHAPES[evidence.sourceShape]} in the source)`;
	}
	if (typeof evidence.occurrences === 'number') {
		return `${head} (defined ${evidence.occurrences}×)`;
	}
	if (typeof evidence.construct === 'string') {
		return `${head} (${evidence.construct} at offset ${evidence.offset})`;
	}
	return head;
}

function plural(count: number, one: string, many: string): string {
	return `${count} ${count === 1 ? one : many}`;
}
