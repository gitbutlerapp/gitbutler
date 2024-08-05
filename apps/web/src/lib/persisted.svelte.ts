/**
 * @link https://github.com/svecosystem/runed/pull/113
 * @link https://runed.dev/docs/utilities/persisted-state
 */

type Serializer<T> = {
	serialize: (value: T) => string;
	deserialize: (value: string) => T;
};

type GetValueFromStorageResult<T> =
	| {
			found: true;
			value: T;
	  }
	| {
			found: false;
			value: null;
	  };

function getValueFromStorage<T>({
	key,
	storage,
	serializer
}: {
	key: string;
	storage: Storage | null;
	serializer: Serializer<T>;
}): GetValueFromStorageResult<T> {
	if (!storage) {
		return { found: false, value: null };
	}

	const value = storage.getItem(key);
	if (value === null) {
		return { found: false, value: null };
	}

	try {
		return {
			found: true,
			value: serializer.deserialize(value)
		};
	} catch (e) {
		console.error(`Error when parsing ${value} from persisted store "${key}"`, e);
		return {
			found: false,
			value: null
		};
	}
}

function setValueToStorage<T>({
	key,
	value,
	storage,
	serializer
}: {
	key: string;
	value: T;
	storage: Storage | null;
	serializer: Serializer<T>;
}) {
	if (!storage) {
		return;
	}

	try {
		storage.setItem(key, serializer.serialize(value));
	} catch (e) {
		console.error(`Error when writing value from persisted store "${key}" to ${storage}`, e);
	}
}

function getStorage(): Storage | null {
	if (typeof window === 'undefined') {
		return null;
	}

	return localStorage;
}

type PersistedStateOptions<T> = {
	/** The serializer to use. Defaults to `JSON.stringify` and `JSON.parse`. */
	serializer?: Serializer<T>;
};

/**
 * Creates reactive state that is persisted and synchronized across browser sessions and tabs using Web Storage.
 * @param key The unique key used to store the state in the storage.
 * @param initialValue The initial value of the state if not already present in the storage.
 * @param options Configuration options including storage type, serializer for complex data types, and whether to sync state changes across tabs.
 */
export class PersistedState<T> {
	#current = $state() as T;
	#key: string;
	#storage: Storage | null;
	#serializer: Serializer<T>;

	constructor(key: string, initialValue: T, options: PersistedStateOptions<T> = {}) {
		const { serializer = { serialize: JSON.stringify, deserialize: JSON.parse } } = options;

		this.#key = key;
		this.#storage = getStorage();
		this.#serializer = serializer;

		const valueFromStorage = getValueFromStorage({
			key: this.#key,
			storage: this.#storage,
			serializer: this.#serializer
		});

		this.#current = valueFromStorage.found ? valueFromStorage.value : initialValue;

		$effect(() => {
			setValueToStorage({
				key: this.#key,
				value: this.#current,
				storage: this.#storage,
				serializer: this.#serializer
			});
		});
	}

	get current(): T {
		return this.#current;
	}

	set current(newValue: T) {
		this.#current = newValue;
	}
}
