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

export type Reactive<T> = { readonly current: T };
export type WritableReactive<T> = { current: T };

export async function guardReadableTrue(target: Readable<boolean>): Promise<boolean> {
	return await new Promise((resolve) => {
		const unsubscribe = target.subscribe((value) => {
			if (value) {
				resolve(value);

				setTimeout(() => {
					unsubscribe();
				}, 0);
			}
		});
	});
}

/**
 * Converts an async subscriber contract to one that can be consumed syncronusly
 * in something like an `$effect`.
 *
 * This function ensures eventual consistency with the state of the effect.
 *
 * It also ensures that a subscribe call is always followed by an unsubscribe,
 * and vice versa. As such, we can guarantee that the subscription counter
 * contained in the async fn is consistent.
 *
 * To manage this, track some important states:
 * - `inUse` tracks the state that the sync world wants us to be in.
 * - `subscribed` tracks whether we are actually subscribed or not.
 * - `working` tracks whether we are currently transitioning from subscribed
 *   to unsubscribed, or vice versa.
 *
 * Whenever the sync world signals a state change, we call an internal
 * `resolveState` function, which checks to see if we're already changing states
 * and if so, simply does nothing. It also checks to see if we're already in
 * the desired state, and also will do nothing.
 *
 * We also call `resolveState` after we have finished either subscribing or
 * unsubscribing which allows us to catch up on any state changes that may of
 * happened while that subscribe or unsubscribe was still happening.
 */
export function asyncToSyncSignals<Args extends [...unknown[]]>(
	fn: (...args: Args) => Promise<void | (() => Promise<void>)>
): (...args: Args) => () => void {
	let inUse = false;
	let working = false;
	let subscribed = false;
	let unsubscribeClosure: (() => Promise<void>) | undefined;

	// Starts the async subscription start.
	//
	// At the end of subscription, `resolveState` gets called again in order to
	// resolve any pending state changes, like if `inUse` got set to false
	// wilst the subscription was in progress.
	function subscribe(args: Args) {
		working = true;

		fn(...args).then((u) => {
			working = false;
			subscribed = true;
			unsubscribeClosure = u || (async () => undefined);

			resolveState(args);
		});
	}

	// Works the same as `subscribe`, but unsubscribes from the signal instead.
	function unsubscribe(args: Args) {
		// In reality we should never return early because if we're still
		// subscribing, then `resolveState` should not call either
		// `subscribe` or `unsubscribe`
		if (!unsubscribeClosure) return;

		working = true;

		unsubscribeClosure().then(() => {
			working = false;
			subscribed = false;
			unsubscribeClosure = undefined;

			resolveState(args);
		});
	}

	// Checks if a new state transition trigger is required.
	//
	function resolveState(args: Args) {
		// If working is true, then there is already a thread working on
		// subscribing or unsubscribing. Even if it was currently subscribing
		// and this was called from the sync unsubscriber, we still don't want
		// to do anything. This is because at the end of that subscribing
		// process, it will call resolveState() again for, and potentially
		// change state again.
		if (working) return;
		if (inUse) {
			if (!subscribed) {
				subscribe(args);
			}
		} else {
			if (subscribed) {
				unsubscribe(args);
			}
		}
	}

	// In the sync functions, we update the state that the sync code wants us
	// to be in (`inUse`), and then call resolveState() which will trigger a
	// subscribe or unsubscribe if it's not already working and if it's needed
	// to resolve the state.
	return (...args: Args) => {
		inUse = true;

		resolveState(args);

		return () => {
			inUse = false;

			resolveState(args);
		};
	};
}
