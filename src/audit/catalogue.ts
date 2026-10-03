import { type Node, parseJson } from './json';

/**
 * Reading one catalogue — the crate's `catalogue.rs`.
 *
 * **Nested and flat are the same catalogue**: `{"a":{"b":"x"}}` and
 * `{"a.b":"x"}` both flatten to `a.b`. A key written twice is kept as a
 * duplicate rather than silently folded, and the **last** value is the one
 * compared, because that is what every loader hands the application.
 *
 * This module knows no library. It reports every key, metadata included;
 * which keys are metadata is decided afterwards, because identification reads
 * the raw key set.
 */

/** What a path holds — the thing `structure-mismatch` compares. */
export type Shape = 'text' | 'object' | 'other';

export interface Entry {
	readonly key: string;
	readonly shape: Shape;
	/** Read only to ask whether it is empty and whether it equals the source. */
	readonly text: string | undefined;
}

export interface Duplicate {
	readonly key: string;
	readonly occurrences: number;
}

export interface Parsed {
	readonly entries: readonly Entry[];
	readonly duplicates: readonly Duplicate[];
}

/** Parse one catalogue, or throw with the reason the crate would give. */
export function parseCatalogue(content: string): Parsed {
	const text = content.startsWith('\uFEFF') ? content.slice(1) : content;
	const node = parseJson(text);
	if (node.kind !== 'object')
		throw new Error('a catalogue must be a JSON object');

	const entries: Entry[] = [];
	const duplicates: Duplicate[] = [];
	walk('', node.entries, entries, duplicates);
	return { entries, duplicates };
}

function walk(
	prefix: string,
	object: ReadonlyArray<readonly [string, Node]>,
	entries: Entry[],
	duplicates: Duplicate[],
): void {
	for (const [name, node] of resolve(prefix, object, duplicates)) {
		const key = path(prefix, name);
		if (node.kind === 'text') {
			entries.push({ key, shape: 'text', text: node.text });
			continue;
		}
		if (node.kind === 'other') {
			entries.push({ key, shape: 'other', text: undefined });
			continue;
		}
		entries.push({ key, shape: 'object', text: undefined });
		walk(key, node.entries, entries, duplicates);
	}
}

function path(prefix: string, name: string): string {
	return prefix === '' ? name : `${prefix}.${name}`;
}

/** One entry per distinct name, in first-appearance order, carrying the last value. */
function resolve(
	prefix: string,
	object: ReadonlyArray<readonly [string, Node]>,
	duplicates: Duplicate[],
): Array<[string, Node]> {
	const resolved: Array<[string, Node]> = [];
	const occurrences: number[] = [];
	const first = new Map<string, number>();

	for (const [name, node] of object) {
		const at = first.get(name);
		if (at !== undefined) {
			(resolved[at] as [string, Node])[1] = node;
			occurrences[at] = (occurrences[at] as number) + 1;
			continue;
		}
		first.set(name, resolved.length);
		resolved.push([name, node]);
		occurrences.push(1);
	}

	resolved.forEach(([name], index) => {
		const count = occurrences[index] as number;
		if (count < 2) return;
		duplicates.push({ key: path(prefix, name), occurrences: count });
	});
	return resolved;
}
