/**
 * This module contains some utilities primarily inspired by RxJS that can
 * help us avoid manually managing subscriptions and unsubscriptions in
 * application code.
 */

import { writable, type Readable, type Writable } from 'svelte/store';

export function combineLatest<T>(stores: Readable<T>[], initialValue: T): Readable<T> {
	const store = writable(initialValue, (set) => {
		const unsubscribers = stores.map((store) => store.subscribe(set));
		return () => {
			unsubscribers.forEach((unsubscribe) => unsubscribe());
		};
	});

	return store;
}

/**
 * Like a writable, but the startStopNotifier gets re-run whenever the provided store changes.
 *
 * If the source store is changed frequently, consider throttling or debouncing it as it will
 * cause the StartStopNotifier to be called frequently.
 *
 * Example lifecycle:
 *
 * o Readable = 1
 * |
 * o - o writableDerived subscribed (ssn called, passed value 1, ssn returns 3) wD, = 3
 * |   |
 * o   o Readable updated to 2, (ssn unsubscribers called, ssn called, passed value 2, ssn returns 4), wD = 4
 * |   |
 * |   o writable derived set to 6, wD = 6
 * |   |
 * |   o writableDerived unsubscribed (ssn unsubscribers called)
 */
export function writableDerived<A, B>(
	store: Readable<B>,
	initialValue: A,
	startStopNotifier: (derivedValue: B, set: (value: A) => void) => (() => void) | undefined
): Writable<A> {
	const derivedStore = writable(initialValue, (set) => {
		let startStopUnsubscribe: (() => void) | undefined = undefined;
		const unsubscribeStore = store.subscribe((value) => {
			startStopUnsubscribe?.();
			startStopUnsubscribe = startStopNotifier(value, set);
		});

		return () => {
			unsubscribeStore();
			startStopUnsubscribe?.();
		};
	});

	return derivedStore;
}
