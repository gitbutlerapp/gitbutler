import { derived, type Readable } from 'svelte/store';

export function flattenPromises<T>(readable: Readable<Promise<T>>): Readable<T | undefined> {
	return derived(readable, (value, set) => {
		let discarded = false;

		value.then((value) => {
			// Don't try to set after the readable has been disposed of
			if (discarded) return;
			set(value);
		});

		return () => {
			discarded = true;
		};
	});
}
