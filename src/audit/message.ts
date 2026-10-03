import type { Grammar, Interpolation, Mark } from './library';

/**
 * Reading one message under a **known** grammar — the crate's `message.rs`.
 *
 * The parser is told what the braces mean; it never classifies. `{name}` is an
 * ICU placeholder and literal text in i18next; `{{ name }}` is an i18next
 * variable and an ICU literal brace around a word. Being told settles it.
 *
 * It scans UTF-8 bytes, as the crate does, so a `convention-mismatch` offset is
 * a byte offset and no continuation byte is ever mistaken for a delimiter.
 */

/** The CLDR categories a key-suffix plural model writes. */
export const PLURAL_SUFFIXES = Object.freeze([
	'_zero',
	'_one',
	'_two',
	'_few',
	'_many',
	'_other',
]);

export type Construct =
	| 'icu-argument'
	| 'fluent'
	| 'printf'
	| 'template-literal'
	| 'nesting';

export interface Foreign {
	readonly construct: Construct;
	/** Byte offset into the message. */
	readonly offset: number;
}

export interface ParsedMessage {
	readonly tokens: readonly string[];
	readonly foreign: readonly Foreign[];
	/** Interpolations in a style this library does not read. */
	readonly others: readonly Interpolation[];
}

interface Draft {
	tokens: string[];
	foreign: Foreign[];
	others: Interpolation[];
}

const encoder = new TextEncoder();
// `ignoreBOM`, or a string that begins with U+FEFF loses it: the decoder
// treats a leading byte-order mark as framing, and here it is content.
const decoder = new TextDecoder('utf-8', { ignoreBOM: true });

const LBRACE = 0x7b;
const RBRACE = 0x7d;
const PERCENT = 0x25;
const DOLLAR = 0x24;

export function parseMessage(grammar: Grammar, text: string): ParsedMessage {
	const bytes = encoder.encode(text);
	const parsed: Draft = { tokens: [], foreign: [], others: [] };
	let index = 0;
	while (index < bytes.length) {
		index += step(bytes, index, grammar, parsed);
	}
	return parsed;
}

function step(
	bytes: Uint8Array,
	index: number,
	grammar: Grammar,
	parsed: Draft,
): number {
	const byte = bytes[index];
	const next = bytes[index + 1];
	if (byte === LBRACE && next === LBRACE)
		return doubleBrace(bytes, index, grammar, parsed);
	if (byte === LBRACE) return singleBrace(bytes, index, grammar, parsed);
	// An ICU literal `}`, stepped over as a pair.
	if (byte === RBRACE && next === RBRACE) return 2;
	// A literal percent: without this, "100%% off" is judged for nothing.
	if (byte === PERCENT && next === PERCENT) return 2;
	if (byte === PERCENT) return percent(bytes, index, parsed);
	if (byte === DOLLAR) return dollar(bytes, index, grammar, parsed);
	return 1;
}

/** `{{name}}` — a variable where that is the grammar, ICU's literal `{` elsewhere. */
function doubleBrace(
	bytes: Uint8Array,
	index: number,
	grammar: Grammar,
	parsed: Draft,
): number {
	if (grammar.interpolation !== 'double-brace') return 2;
	const start = index + 2;
	const end = identifierEnd(bytes, start);
	if (end !== undefined && bytes[end] === RBRACE && bytes[end + 1] === RBRACE) {
		parsed.tokens.push(name(bytes, start, end));
		return end + 2 - index;
	}
	if (!grammar.trims) return 2;
	// `{{ name }}`: i18next trims, so this is the variable `name`.
	const opened = skipSpace(bytes, start);
	const trimmedEnd = identifierEnd(bytes, opened);
	if (trimmedEnd === undefined) return 2;
	const closing = skipSpace(bytes, trimmedEnd);
	if (!(bytes[closing] === RBRACE && bytes[closing + 1] === RBRACE)) return 2;
	parsed.tokens.push(name(bytes, opened, trimmedEnd));
	return closing + 2 - index;
}

function singleBrace(
	bytes: Uint8Array,
	index: number,
	grammar: Grammar,
	parsed: Draft,
): number {
	const opened = index + 1;
	const start = grammar.trims ? skipSpace(bytes, opened) : opened;
	if (bytes[start] === DOLLAR) {
		// No library here writes Fluent, so this is foreign under all of them.
		parsed.foreign.push({ construct: 'fluent', offset: index });
		return 1;
	}

	const end = identifierEnd(bytes, start);
	if (end === undefined) return 1;
	const closing = grammar.trims ? skipSpace(bytes, end) : end;

	if (bytes[closing] === 0x2c)
		return icuArgument(bytes, index, start, end, grammar, parsed);
	if (bytes[closing] !== RBRACE) return 1;

	const found: Interpolation = allDigits(bytes, start, end)
		? 'positional'
		: 'single-brace';
	if (reads(grammar.interpolation, found)) {
		parsed.tokens.push(name(bytes, start, end));
		return closing + 1 - index;
	}

	// Not this library's interpolation: prose on its own, a finding only where
	// the source had a real placeholder at the same key.
	if (!parsed.others.includes(found)) parsed.others.push(found);
	return closing + 1 - index;
}

function reads(grammar: Interpolation, found: Interpolation): boolean {
	switch (grammar) {
		case 'single-brace':
			return true; // ICU reads `{0}` and `{name}` alike.
		case 'positional':
			return found === 'positional';
		case 'double-brace':
			return false;
	}
}

/** `{count, plural, ...}`: one token where ICU is native, a defect where it is not. */
function icuArgument(
	bytes: Uint8Array,
	index: number,
	start: number,
	end: number,
	grammar: Grammar,
	parsed: Draft,
): number {
	if (grammar.icu) parsed.tokens.push(name(bytes, start, end));
	else parsed.foreign.push({ construct: 'icu-argument', offset: index });
	return balanced(bytes, index) ?? 1;
}

/** How far past `index` the brace opened there closes, or undefined. */
function balanced(bytes: Uint8Array, index: number): number | undefined {
	let depth = 0;
	for (let offset = index; offset < bytes.length; offset++) {
		const byte = bytes[offset];
		if (byte === LBRACE) depth++;
		if (byte !== RBRACE) continue;
		if (depth === 0) return undefined;
		depth--;
		if (depth === 0) return offset + 1 - index;
	}
	return undefined;
}

/**
 * `%s`, `%d`, `%1$s` — a conversion straight after the `%`, or a positional
 * argument before it. Flags, width and precision are deliberately not read:
 * Hungarian "90%-os" would otherwise be a printf conversion.
 */
function percent(bytes: Uint8Array, index: number, parsed: Draft): number {
	let cursor = index + 1;
	const digits = cursor;
	while (isDigit(bytes[cursor])) cursor++;
	if (cursor > digits) {
		if (bytes[cursor] !== DOLLAR) return 1;
		cursor++;
	}
	const conversion = bytes[cursor];
	if (conversion === undefined || !PRINTF_CONVERSIONS.has(conversion)) return 1;
	parsed.foreign.push({ construct: 'printf', offset: index });
	return 1;
}

const PRINTF_CONVERSIONS: ReadonlySet<number> = new Set(
	Array.from('sdiufFgGeExXoc', (c) => c.charCodeAt(0)),
);

function dollar(
	bytes: Uint8Array,
	index: number,
	grammar: Grammar,
	parsed: Draft,
): number {
	if (bytes[index + 1] === LBRACE) {
		parsed.foreign.push({ construct: 'template-literal', offset: index });
		return 2;
	}
	if (bytes[index + 1] === 0x74 && bytes[index + 2] === 0x28) {
		// Where nesting is native it is a key reference, and the scan continues
		// past it so an interpolation inside the referenced key still counts.
		if (!grammar.nesting)
			parsed.foreign.push({ construct: 'nesting', offset: index });
		return 3;
	}
	return 1;
}

/** `.`, `-` and `_` are inside a name: `{{user.name}}` is ordinary. */
function identifierEnd(bytes: Uint8Array, start: number): number | undefined {
	const first = bytes[start];
	if (first === undefined || !(isAlphanumeric(first) || first === 0x5f))
		return undefined;
	let end = start;
	for (;;) {
		const byte = bytes[end];
		if (byte === undefined) return end;
		if (
			!(isAlphanumeric(byte) || byte === 0x5f || byte === 0x2e || byte === 0x2d)
		)
			return end;
		end++;
	}
}

function skipSpace(bytes: Uint8Array, from: number): number {
	let index = from;
	while (isAsciiWhitespace(bytes[index])) index++;
	return index;
}

function name(bytes: Uint8Array, start: number, end: number): string {
	return decoder.decode(bytes.subarray(start, end));
}

function allDigits(bytes: Uint8Array, start: number, end: number): boolean {
	for (let k = start; k < end; k++) if (!isDigit(bytes[k])) return false;
	return true;
}

function isDigit(byte: number | undefined): boolean {
	return byte !== undefined && byte >= 0x30 && byte <= 0x39;
}

function isAlphanumeric(byte: number): boolean {
	return (
		isDigit(byte) ||
		(byte >= 0x41 && byte <= 0x5a) ||
		(byte >= 0x61 && byte <= 0x7a)
	);
}

/** `u8::is_ascii_whitespace`: space, tab, LF, FF, CR — not VT. */
function isAsciiWhitespace(byte: number | undefined): boolean {
	return (
		byte === 0x20 ||
		byte === 0x09 ||
		byte === 0x0a ||
		byte === 0x0c ||
		byte === 0x0d
	);
}

/** A key's plural base, where the library writes plurals as suffixes. */
export function pluralBase(key: string): string {
	for (const suffix of PLURAL_SUFFIXES) {
		if (key.endsWith(suffix)) return key.slice(0, -suffix.length);
	}
	return key;
}

// ---------------------------------------------------------------------------
// Identification: grammar-free predicates, used only to answer "which library
// wrote this". Nothing here reads a message in order to report on it.
// ---------------------------------------------------------------------------

export function marks(text: string): Mark[] {
	const bytes = encoder.encode(text);
	const found: Mark[] = [];
	let index = 0;
	while (index < bytes.length) {
		const [mark, advanced] = markAt(bytes, index);
		if (mark !== undefined && !found.includes(mark)) found.push(mark);
		index += advanced;
	}
	return found;
}

function markAt(bytes: Uint8Array, index: number): [Mark | undefined, number] {
	if (bytes[index] === LBRACE && bytes[index + 1] === LBRACE) {
		const opened = skipSpace(bytes, index + 2);
		return [
			identifierEnd(bytes, opened) === undefined ? undefined : 'double-brace',
			2,
		];
	}
	if (bytes[index] === LBRACE) return [braceMark(bytes, index), 1];
	if (
		bytes[index] === DOLLAR &&
		bytes[index + 1] === 0x74 &&
		bytes[index + 2] === 0x28
	)
		return ['dollar-t', 3];
	return [undefined, 1];
}

function braceMark(bytes: Uint8Array, index: number): Mark | undefined {
	const start = skipSpace(bytes, index + 1);
	const end = identifierEnd(bytes, start);
	if (end === undefined) return undefined;
	const closing = skipSpace(bytes, end);
	if (bytes[closing] === 0x2c) return 'icu-argument';
	if (bytes[closing] !== RBRACE) return undefined;
	return allDigits(bytes, start, end) ? 'positional' : 'single-brace';
}

/** Marks that live in the key set rather than in a message. */
export function keyMarks(keys: readonly string[]): Mark[] {
	const found: Mark[] = [];
	if (hasPluralFamily(keys)) found.push('plural-key-suffix');

	const keySet = new Set(keys);
	const hasDocumentMetadata = keys.some((key) => key.startsWith('@@'));
	const hasSiblingMetadata = keys.some((key) => {
		if (!key.startsWith('@')) return false;
		const rest = key.slice(1);
		return !rest.startsWith('@') && !rest.includes('.') && keySet.has(rest);
	});
	if (hasDocumentMetadata && hasSiblingMetadata) found.push('arb-metadata');

	// A dotted identifier never contains a space; an English sentence does.
	const sentences = keys.filter(
		(key) => !key.startsWith('@') && key.includes(' '),
	).length;
	if (sentences * 2 > keys.length && sentences > 0) found.push('sentence-keys');
	return found;
}

/** `item_one` beside `item_other`. One variant alone is not a family. */
function hasPluralFamily(keys: readonly string[]): boolean {
	const bases: string[] = [];
	for (const key of keys) {
		const base = pluralBase(key);
		if (base === key) continue;
		if (bases.includes(base)) return true;
		bases.push(base);
	}
	return false;
}
