/**
 * The string questions the crate asks, answered the way Rust answers them.
 *
 * `str::trim` strips Unicode `White_Space`, which is not JavaScript's set:
 * U+0085 is whitespace to Rust and not to `String.prototype.trim`, and U+FEFF
 * is the reverse. An `empty-value` finding turns on that difference, so it is
 * decided here rather than by the engine.
 */
const WHITE_SPACE =
	/^[\t\n\v\f\r \u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]*$/;

/** Whether `str::trim(text).is_empty()` would be true. */
export function isBlank(text: string): boolean {
	return WHITE_SPACE.test(text);
}

/** ASCII letters only, as `u8::is_ascii_alphabetic` reads. */
export function isAsciiAlphabetic(text: string): boolean {
	return /^[A-Za-z]*$/.test(text);
}

export function isAsciiDigits(text: string): boolean {
	return /^[0-9]*$/.test(text);
}

/** How many bytes `text` is in UTF-8, which is what Rust's `len()` counts. */
export function byteLength(text: string): number {
	return new TextEncoder().encode(text).length;
}

const LEADING =
	/^[\t\n\v\f\r \u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+/;
const TRAILING =
	/[\t\n\v\f\r \u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+$/;

/** `str::trim`, with Rust's whitespace. */
export function trim(text: string): string {
	return text.replace(LEADING, '').replace(TRAILING, '');
}

/** `str::trim_end`, with Rust's whitespace. */
export function trimEnd(text: string): string {
	return text.replace(TRAILING, '');
}

/** Whether the first character is whitespace as `char::is_whitespace` reads it. */
export function startsWithWhitespace(text: string): boolean {
	return LEADING.test(text);
}

/** `str::lines`: split on LF, dropping a CR only where an LF follows it. */
export function lines(text: string): string[] {
	const segments = text.split('\n');
	const last = segments.pop() as string;
	const out = segments.map((line) =>
		line.endsWith('\r') ? line.slice(0, -1) : line,
	);
	if (last !== '') out.push(last);
	return out;
}
