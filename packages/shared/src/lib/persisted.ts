/**
 * This is simplified copy of the persisted store in square/svelte-store.
 */
import lscache from 'lscache';
import { writable, type Writable } from 'svelte/store';
export type Persisted<T> = Writable<T> & { synchronize: () => void };

// Using a WeakRef means that if all the users of the persisted go away, the
// objects can get correctly GCed.
const persistedInstances = new Map<string, WeakRef<Persisted<unknown>>>();
const persistedInstancesWithExpiration = new Map<string, WeakRef<Persisted<unknown>>>();

export function getStorageItem(key: string): unknown {
	const item = window.localStorage.getItem(key);
	try {
		return item ? JSON.parse(item) : undefined;
	} catch {
		return undefined;
	}
}

export function getBooleanStorageItem(key: string): boolean | undefined {
	const item = getStorageItem(key);
	if (typeof item === 'boolean') {
		return item;
	}
	return undefined;
}

export function setStorageItem(key: string, value: unknown): void {
	window.localStorage.setItem(key, JSON.stringify(value));
}

export function setBooleanStorageItem(key: string, value: boolean): void {
	setStorageItem(key, value);
}

export function removeStorageItem(key: string): void {
	window.localStorage.removeItem(key);
}

export function persisted<T>(initial: T, key: string): Persisted<T> {
	// Check if we already have an instance for this key
	const instance = persistedInstances.get(key)?.deref();
	if (instance) {
		instance.synchronize();
		return instance as Persisted<T>;
	}

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

	const persistedStore = {
		subscribe,
		set,
		update,
		synchronize: () => synchronize(thisStore.set)
	};

	// Store the instance with a WeakRef
	persistedInstances.set(key, new WeakRef(persistedStore));

	return persistedStore;
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

export function persistWithExpiration<T>(
	initial: T,
	key: string,
	expirationInMinutes: number
): Persisted<T> {
	// Check if we already have an instance for this key
	const instance = persistedInstancesWithExpiration.get(key)?.deref();
	if (instance) {
		instance.synchronize();
		return instance as Persisted<T>;
	}

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

	const persistedStore = {
		subscribe,
		set,
		update,
		synchronize: () => synchronize(thisStore.set)
	};

	persistedInstancesWithExpiration.set(key, new WeakRef(persistedStore));

	return persistedStore;
}
