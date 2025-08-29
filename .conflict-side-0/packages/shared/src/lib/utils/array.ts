export function deduplicate<T>(array: T[]): T[] {
	return Array.from(new Set(array));
}

export function deduplicateBy<T, K extends keyof T>(array: T[], key: K): T[] {
	const seen = new Set();
	const result: T[] = [];

	for (const item of array) {
		const value = item[key];
		if (!seen.has(value)) {
			seen.add(value);
			result.push(item);
		}
	}

	return result;
}

export function filterWithRest<T>(array: T[], predicate: (item: T) => boolean): [T[], T[]] {
	const rest: T[] = [];
	const filtered: T[] = [];

	for (const item of array) {
		if (predicate(item)) {
			filtered.push(item);
			continue;
		}
		rest.push(item);
	}

	return [filtered, rest];
}

export function flattenAndDeduplicate<T>(array: T[][]): T[] {
	return deduplicate(array.flat());
}
