import type { Readable, Subscriber, Unsubscriber, Writable } from 'svelte/store';
import * as vanilla from 'svelte/store';

type Loading = { isLoading: true };

type Loaded<T> = { isLoading: false; value: T };

type Loadable<T> = Loading | Loaded<T>;

type StartStopNotifier<T> = (set: Subscriber<T>) => Promise<Unsubscriber> | Unsubscriber | void;

const noop = () => {};

const isPromise = <T>(value: T | Promise<T>): value is Promise<T> => {
	return value && typeof (value as Promise<T>).then === 'function';
};

// writable is a wrapper for vanilla svelte writable store.
// it can be used to create a store that is populated asynchronously.
// the store will be in a loading state until the promise resolves.
// it is also possible to pass a startStopNotifier function that will be called when the store is subscribed to.
// this can be used to start and stop a subscription to ipc events.
export const writable = <T>(
	value?: T | Promise<T>,
	start: StartStopNotifier<T> = noop
): Writable<Loadable<T>> => {
	const isValuePromised = isPromise(value);

	const initialValue =
		value === undefined || isValuePromised
			? ({ isLoading: true } as Loading)
			: ({ isLoading: false, value } as Loaded<T>);
	const { set, update, subscribe } = vanilla.writable<Loadable<T>>(
		initialValue,
		// allow the user to subscribe to the store
		(set) => {
			const stop = () => {};
			Promise.resolve(start((value) => set({ isLoading: false, value }))).then(stop);
			return stop;
		}
	);

	if (isValuePromised) {
		// when the promise resolves, the store will be updated with the value.
		value.then((value) =>
			// if the store is already loading, we don't want to overwrite the value.
			update((state) => (state.isLoading ? { isLoading: false, value } : state))
		);
	}

	return { set, update, subscribe };
};

type Stores<T> =
	| Readable<Loadable<T>>
	| [Readable<Loadable<T>>, ...Array<Readable<Loadable<T>>>]
	| Array<Readable<Loadable<T>>>;

type StoresValues<T> = T extends Readable<Loadable<infer U>>
	? U
	: {
			[K in keyof T]: T[K] extends Readable<Loadable<infer U>> ? U : never;
	  };

// derived works similar to how it works in vanilla svelte.
// it can be used to create an asynchronously populated store from other asynchronously populated stores.
// the store will be in a loading state until all the dependencies are resolved.
export const derived = <S extends Stores<any>, T>(
	stores: S,
	fn: (values: StoresValues<S>) => T
): Readable<Loadable<T>> => {
	const single = !Array.isArray(stores);
	const stores_array: Array<Readable<Loadable<T>>> = single
		? [stores as Readable<Loadable<T>>]
		: (stores as Array<Readable<Loadable<T>>>);

	return vanilla.readable<Loadable<T>>({ isLoading: true }, (set) => {
		let loaded = 0;
		const values = new Array<Loaded<T>>(stores_array.length);
		const unsubscribers = new Array(stores_array.length);

		const update = () => {
			if (loaded === values.length) {
				set({
					isLoading: false,
					value: fn(single ? values[0].value : (values.map(({ value }) => value) as any))
				});
			}
		};

		stores_array.forEach((store, i) => {
			unsubscribers[i] = store.subscribe((value) => {
				if (!value.isLoading) {
					values[i] = value;
					loaded += 1;
					update();
				}
			});
		});

		return () => {
			unsubscribers.forEach((unsubscribe) => unsubscribe());
		};
	});
};

export const readable = <T>(
	value: T | Promise<T>,
	start: StartStopNotifier<T> = noop
): Readable<Loadable<T>> => writable(value, start);
