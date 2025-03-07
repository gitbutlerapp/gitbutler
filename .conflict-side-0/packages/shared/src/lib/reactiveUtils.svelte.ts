import { get, type Readable, type Writable } from 'svelte/store';
import type { Reactive, WritableReactive } from '$lib/storeUtils';

export function reactive<T>(fn: () => T): Reactive<T> {
	return {
		get current() {
			return fn();
		}
	};
}

export function readableToReactive<T>(readable?: Readable<T>): Reactive<T | undefined> {
	let current = $state<T | undefined>(readable ? get(readable) : undefined);

	$effect(() => {
		if (readable) {
			current = get(readable);
		}

		const unsubscribe = readable?.subscribe((value) => {
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
	let current = $state<T | undefined>(writable ? get(writable) : undefined);

	$effect(() => {
		if (writable) {
			current = get(writable);
		}

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
