/** Ponyfill of `Iterator.concat`, unavailable until Node.js v26. */
export function iteratorConcat<T>(...iterables: Array<Iterable<T>>): IteratorObject<T> {
	return (function* (): Generator<T> {
		for (const iterable of iterables) yield* iterable;
	})();
}

/** Like `Array.prototype.values`, but iterates in reverse order. */
export function* reverseValues<T>(array: Array<T>): Generator<T> {
	for (let index = array.length - 1; index >= 0; index--)
		// oxlint-disable-next-line typescript/no-non-null-assertion
		yield array[index]!;
}
