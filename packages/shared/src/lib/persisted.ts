/**
 * This is simplified copy of the persisted store in square/svelte-store.
 */
import lscache from 'lscache';
import { writable, type Writable } from 'svelte/store';
export type Persisted<T> = Writable<T>;

export function getStorageItem(key: string): unknown {
	const item = window.localStorage.getItem(key);
	try {
		return item ? JSON.parse(item) : undefined;
	} catch {
		return undefined;
	}
}

export function setStorageItem(key: string, value: unknown): void {
	window.localStorage.setItem(key, JSON.stringify(value));
}

export function persisted<T>(initial: T, key: string): Persisted<T> {
	function setAndPersist(value: T, set: (value: T) => void) {
		setStorageItem(key, value);
		set(value);
	}

	function synchronize(set: (value: T) => void): void {
		const stored = getStorageItem(key);
		if (stored !== undefined) {
			set(stored as T);
		} else {
			setAndPersist(initial, set);
		}
	}

	function update() {
		throw 'Not implemented';
	}

	const thisStore = writable<T>(initial, (set) => {
		synchronize(set);
	});

	async function set(value: T) {
		setAndPersist(value, thisStore.set);
	}

	const subscribe = thisStore.subscribe;

	return {
		subscribe,
		set,
		update
	};
}

export function setEphemeralStorageItem(
	key: string,
	value: unknown,
	expirationInMinutes: number
): void {
	lscache.set(key, JSON.stringify(value), expirationInMinutes);
}

export function getEphemeralStorageItem(key: string): unknown {
	const item = lscache.get(key);
	try {
		if (!item) {
			return undefined;
		}

		const parsed = JSON.parse(item);
		return parsed;
	} catch {
		return undefined;
	}
}

/**
 * Create a persisted store that expires after a certain amount of time (in minutes).
 */
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
export function persistWithExpiration<T extends {}>(
	initial: T,
	key: string,
	expirationInMinutes: number
): Persisted<T> {
	function setAndPersist(value: T, set: (value: T) => void) {
		setEphemeralStorageItem(key, value, expirationInMinutes);
		set(value);
	}

	function synchronize(set: (value: T) => void): void {
		const stored = getEphemeralStorageItem(key);
		if (stored !== undefined) {
			set(stored as T);
		} else {
			setAndPersist(initial, set);
		}
	}

	function update() {
		throw 'Not implemented';
	}

	const thisStore = writable<T>(initial, (set) => {
		synchronize(set);
	});

	async function set(value: T) {
		setAndPersist(value, thisStore.set);
	}

	const subscribe = thisStore.subscribe;

	return {
		subscribe,
		set,
		update
	};
}
