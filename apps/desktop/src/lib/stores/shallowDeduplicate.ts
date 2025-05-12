import { shallowCompare } from '@gitbutler/shared/shallowCompare';
import { writable, type Readable } from 'svelte/store';

export function shallowDeduplicate<T>(source: Readable<T>) {
	return new ShallowDeduplicateDervived<T>(source);
}

/**
 * A derived Svelte store that only emits new values if the objects differ
 * from each other using `shallowCompare`.
 */
class ShallowDeduplicateDervived<T> implements Readable<T> {
	/**
	 * Previously set value used for comparison with new values.
	 */
	private prev: T | undefined;

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
