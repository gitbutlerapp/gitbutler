import type { Reactive } from '$lib/storeUtils';
import type { Readable } from 'svelte/store';

export function readableToReactive<T>(readable: Readable<T>): Reactive<T | undefined> {
	let current = $state<T>();

	$effect(() => {
		const unsubscribe = readable.subscribe((value) => {
			current = value;
		});

		return unsubscribe;
	});

	return {
		get current() {
			return current;
		}
	};
}
