import { writable, type Readable } from 'svelte/store';

export function uniqueDerived<T>(source: Readable<T>) {
	return new UniqueDerived<T>(source);
}

/**
 * A derived Svelte store that only emits new values if the objects differ
 * from each other using `shallowCompare`.
 */
class UniqueDerived<T> implements Readable<T> {
	/**
	 * Previously set value used for comparison with new values.
	 */
	private prev: unknown;

	/**
	 * A writable that sets up the parent subscription when it starts, and
	 * unsubscribes when last subscriber leaves.
	 */
	private store = writable<T>(undefined, (set) => {
		const unsubscribe = this.parent.subscribe((value) => {
			if (!shallowCompare(this.prev, value)) {
				set(value);
			}
			this.prev = value;
		});
		return () => {
			unsubscribe();
		};
	});

	constructor(private parent: Readable<T>) {}

	/**
	 * Subscribe call is forwarded to our writable instance.
	 */
	subscribe(run: (value: T) => void, invalidate?: () => void): () => void {
		return this.store.subscribe(
			(value) => {
				run(value);
			},
			() => {
				invalidate?.();
			}
		);
	}
}

function shallowCompare(left: unknown, right: unknown): boolean {
	if (left === right) {
		return true;
	}

	if (typeof left !== 'object' || typeof right !== 'object' || left === null || right === null) {
		return false;
	}

	const keys1 = Object.keys(left);
	const keys2 = Object.keys(right);

	// Compare key lengths
	if (keys1.length !== keys2.length) {
		return false;
	}

	// Compare values for each key
	for (const key of keys1) {
		if ((left as Record<string, any>)[key] !== (right as Record<string, any>)[key]) {
			return false;
		}
	}

	return true;
}
