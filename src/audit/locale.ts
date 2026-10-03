import { byteLength, isAsciiAlphabetic, isAsciiDigits } from './text';

/**
 * Which locale a catalogue is for, read from its file name — the crate's
 * `locale.rs`.
 *
 * **The locale is whatever the file names in a set do not share.** Reading one
 * name alone cannot work: `nls` in `package.nls.json` passes for a language
 * tag, and `app_en.arb` is either the locale `app-EN` or the prefix `app` and
 * the locale `en`. Only the set decides, and a leftover segment that is not
 * shaped like a tag is refused rather than guessed at.
 */

/** The canonical spelling of a language tag, or undefined when it is not one. */
export function canonicalise(tag: string): string | undefined {
	const subtags = tag.split('-');
	let position = 0;

	const language = subtags[position++] as string;
	const languageLength = byteLength(language);
	if (languageLength < 2 || languageLength > 3 || !isAsciiAlphabetic(language))
		return undefined;
	let canonical = language.toLowerCase();

	let next = subtags[position++];
	if (next !== undefined && isScript(next)) {
		canonical += `-${next.slice(0, 1).toUpperCase()}${next.slice(1).toLowerCase()}`;
		next = subtags[position++];
	}
	if (next !== undefined && isRegion(next)) {
		canonical += `-${next.toUpperCase()}`;
		next = subtags[position++];
	}
	return next === undefined ? canonical : undefined;
}

function isScript(part: string): boolean {
	return byteLength(part) === 4 && isAsciiAlphabetic(part);
}

function isRegion(part: string): boolean {
	const length = byteLength(part);
	return (
		(length === 2 && isAsciiAlphabetic(part)) ||
		(length === 3 && isAsciiDigits(part))
	);
}

/**
 * The locale of every file in a set, in the order given. `undefined` is the
 * base catalogue. Throws when a leftover segment is not a tag — the refusal
 * that stops a repository root from being audited as `package.json` against
 * `tsconfig.json`.
 */
export function localesOf(names: readonly string[]): Array<string | undefined> {
	const stems = names.map(stem);
	const shared = sharedPrefix(stems);
	return names.map((name, index) =>
		localeOf(name, (stems[index] as string[]).slice(shared)),
	);
}

function localeOf(
	name: string,
	remainder: readonly string[],
): string | undefined {
	if (remainder.length === 0) return undefined;
	const tag = remainder.join('-');
	const canonical = canonicalise(tag);
	if (canonical === undefined) {
		throw new Error(
			`${name}: "${tag}" is not a language tag, so this is not a catalogue set`,
		);
	}
	return canonical;
}

/**
 * The locale of a file whose library fixes the prefix — `package.nls.de.json`
 * under `package.nls` — which is what lets one name be read on its own.
 */
export function fromPrefix(name: string, prefix: string): string | undefined {
	const bare = withoutExtension(name);
	if (!bare.startsWith(prefix))
		throw new Error(`${name} is not a ${prefix} catalogue`);
	const rest = bare.slice(prefix.length);
	if (rest === '') return undefined;
	const tag = rest.replace(/^[._]+/, '');
	const canonical = canonicalise(tag);
	if (canonical === undefined)
		throw new Error(`${name}: "${tag}" is not a language tag`);
	return canonical;
}

function withoutExtension(name: string): string {
	if (name.endsWith('.json')) return name.slice(0, -'.json'.length);
	if (name.endsWith('.arb')) return name.slice(0, -'.arb'.length);
	return name;
}

/** `_` separates as much as `.` does: both appear in the same conventions. */
function stem(name: string): string[] {
	return withoutExtension(name).split(/[._]/);
}

function sharedPrefix(stems: ReadonlyArray<readonly string[]>): number {
	const [first, ...rest] = stems;
	if (!first) return 0;
	let shared = first.length;
	for (const other of rest) {
		let common = 0;
		while (
			common < first.length &&
			common < other.length &&
			first[common] === other[common]
		)
			common++;
		shared = Math.min(shared, common);
	}
	return shared;
}
