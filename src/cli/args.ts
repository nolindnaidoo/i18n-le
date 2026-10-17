/**
 * The command line, parsed as the Rust CLI parses it: strictly, in order, and
 * refusing rather than defaulting.
 */
import { type LibraryId, libraryNames, parseLibrary } from '../audit/library';
import { FLAGS } from './usage';

/** What fails the run besides the findings that already do. */
export type FailOn = 'findings' | 'untranslated' | 'any';

export interface Cli {
	readonly inputs: readonly string[];
	readonly named: LibraryId | undefined;
	readonly source: string | undefined;
	readonly keysAreSource: boolean;
	readonly failOn: FailOn;
	readonly strict: boolean;
}

type Draft = { -readonly [K in keyof Cli]: Cli[K] };

export function parse(args: readonly string[]): Cli {
	const draft: Draft = {
		inputs: [],
		named: undefined,
		source: undefined,
		keysAreSource: false,
		failOn: 'findings',
		strict: false,
	};

	for (let index = 0; index < args.length; index++) {
		const arg = args[index] as string;
		if (arg.startsWith('-') && !FLAGS.includes(arg)) {
			throw new Error(`${arg} is not an option. Try --help.`);
		}
		const value = () => {
			index += 1;
			return args[index];
		};
		if (arg === '--keys-are-source') {
			draft.keysAreSource = true;
			continue;
		}
		if (arg === '--strict') {
			draft.strict = true;
			continue;
		}
		if (arg === '--system') {
			draft.named = library(value());
			continue;
		}
		if (arg === '--source') {
			draft.source = source(value());
			continue;
		}
		if (arg === '--fail-on') {
			draft.failOn = failOn(value());
			continue;
		}
		draft.inputs = [...draft.inputs, arg];
	}

	if (draft.inputs.length === 0) {
		throw new Error('name the directory holding the catalogues. Try --help.');
	}
	return Object.freeze(draft);
}

function library(value: string | undefined): LibraryId {
	const names = libraryNames().join(', ');
	if (value === undefined) throw new Error(`--system needs one of ${names}`);
	const parsed = parseLibrary(value);
	if (parsed === undefined) {
		throw new Error(
			`${value} is not a library this reads. Try one of ${names}.`,
		);
	}
	return parsed;
}

function source(value: string | undefined): string {
	if (value === undefined) {
		throw new Error('--source needs a path or a language tag');
	}
	return value;
}

function failOn(value: string | undefined): FailOn {
	if (value === undefined)
		throw new Error('--fail-on needs untranslated or any');
	if (value === 'untranslated' || value === 'any') return value;
	throw new Error(`--fail-on takes untranslated or any, not ${value}`);
}
