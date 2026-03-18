import { Dispatch, SetStateAction, useEffect, useState } from "react";

export type UseState<T> = [T, Dispatch<SetStateAction<T>>];

/**
 * When changing the value type, also change the key to avoid parsing errors
 * from previous values.
 */
export const useLocalStorageState = <T>(key: string, initialState: T): UseState<T> => {
	const [value, setValue] = useState<T>(() => {
		const storedValue = localStorage.getItem(key);
		if (storedValue === null) return initialState;

		try {
			return JSON.parse(storedValue) as T;
		} catch {
			return initialState;
		}
	});

	useEffect(() => {
		const serializedValue = JSON.stringify(value);
		const serializedInitialValue = JSON.stringify(initialState);

		if (serializedValue === serializedInitialValue) {
			localStorage.removeItem(key);
			return;
		}

		localStorage.setItem(key, serializedValue);
	}, [initialState, key, value]);

	return [value, setValue];
};
