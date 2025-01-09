import type { Reactive, WritableReactive } from '$lib/storeUtils';
import type { Readable, Writable } from 'svelte/store';

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

export function writableToReactive<T>(writable: Writable<T>): WritableReactive<T | undefined> {
	let current = $state<T>();

	$effect(() => {
		const unsubscribe = writable.subscribe((value) => {
			current = value;
		});

		return unsubscribe;
	});

	return {
		get current() {
			return current;
		},

		set current(value: T | undefined) {
			if (value !== undefined) {
				writable.set(value);
			}
		}
	};
}
