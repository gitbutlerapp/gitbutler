import { Dispatch, SetStateAction, useEffect, useState } from "react";

export type UseState<T> = [T, Dispatch<SetStateAction<T>>];

/**
 * When changing the value type, also change the key to avoid parsing errors
 * from previous values.
 */
// https://github.com/bvaughn/react-resizable-panels/blob/08cfd6fdd5e9c7bff07b1c27ae34e679a45f7057/src/hooks/useLocalStorage.ts
// https://github.com/uidotdev/usehooks/blob/945436df0037bc21133379a5e13f1bd73f1ffc36/index.js#L616
// https://github.com/streamich/react-use/blob/9ef95352e459dd2920b0492c63c39863024ee852/src/useLocalStorage.ts
// https://github.com/astoilkov/use-local-storage-state
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

	// Sync changes to local storage
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
