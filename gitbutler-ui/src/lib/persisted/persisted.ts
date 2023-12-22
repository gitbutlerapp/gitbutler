/**
 * This is simplified copy of the persisted store in square/svelte-store.
 */
import { writable, type Writable } from 'svelte/store';
export type Persisted<T> = Writable<T>;

export const getStorageItem = (key: string): unknown => {
	const item = window.localStorage.getItem(key);
	try {
		return item ? JSON.parse(item) : undefined;
	} catch {
		return undefined;
	}
};

export const setStorageItem = (key: string, value: unknown): void => {
	window.localStorage.setItem(key, JSON.stringify(value));
};

export const persisted = <T>(initial: T, key: string): Persisted<T> => {
	const setAndPersist = (value: T, set: (value: T) => void) => {
		setStorageItem(key, value);
		set(value);
	};

	const synchronize = (set: (value: T) => void): void => {
		const stored = getStorageItem(key);
		if (stored !== undefined) {
			set(stored as T);
		} else {
			setAndPersist(initial, set);
		}
	};

	const update = () => {
		throw 'Not implemented';
	};

	const thisStore = writable<T>(initial, (set) => {
		synchronize(set);
	});

	const set = async (value: T) => {
		setAndPersist(value, thisStore.set);
	};

	const subscribe = thisStore.subscribe;

	return {
		subscribe,
		set,
		update
	};
};
