/**
 * Memoizes a function to cache its results based on the arguments passed.
 */
export function memoize<T extends (...args: any[]) => any>(fn: T): T {
	const cache = new Map<string, ReturnType<T>>();

	return function (...args: Parameters<T>): ReturnType<T> {
		const key = JSON.stringify(args);
		if (cache.has(key)) {
			return cache.get(key)!;
		}

		const result = fn(...args);
		cache.set(key, result);
		return result;
	} as T;
}
