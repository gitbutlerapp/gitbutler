import { get, type Readable, type Writable } from "svelte/store";
import type { Reactive, WritableReactive } from "$lib/storeUtils";

/**
 * Helper function for passing reactive variables around.
 *
 * To preserve reactivity between .svelte.ts files it is necessary to pass
 * it as an object with a getter method. If the reactive variable is directly
 * referenced then it triggers additional reactivity.
 *
 * TODO: Find (or write) a Svelte docs page describing this behavior.
 */
export function reactive<T>(fn: () => T): Reactive<T> {
	return {
		get current() {
			return fn();
		},
	};
}

export function writableReactive<T>(get: () => T, set: (_: T) => void): WritableReactive<T> {
	return {
		get current() {
			return get();
		},
		set current(value: T) {
			set(value);
		},
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
		},
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
		},
	};
}
