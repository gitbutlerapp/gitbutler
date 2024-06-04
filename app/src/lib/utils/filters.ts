import { isStr } from '$lib/utils/string';

// If a value occurs > 1 times then all but one will fail this condition.
export function unique(value: any, index: number, array: any[]) {
	return array.indexOf(value) === index;
}

/**
 * Filters an array of objects based on a specific string value in a given key.
 *
 * @template T - The type of objects in the array.
 * @template K - The key of the object to filter by.
 * @param {T[]} arr - The array of objects to filter.
 * @param {K} key - The key to filter by.
 * @param {string} value - The string value to search for in the specified key.
 * @returns {T[]} - The filtered array of objects.
 */
export function filterStringByKey<T, K extends keyof T>(arr: T[], key: K, value: string): T[] {
	const result: T[] = [];
	for (const item of arr) {
		const attribute = item[key];
		if (!isStr(attribute)) continue;
		if (attribute.includes(value)) {
			result.push(item);
		}
	}
	return result;
}
