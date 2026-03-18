import { Dispatch, SetStateAction, useEffect, useRef, useState } from "react";

export type UseState<T> = [T, Dispatch<SetStateAction<T>>];

/**
 * When changing the value type, also change the key to avoid parsing errors
 * from previous values.
 */
export const useLocalStorageState = <T>(key: string, initialValue: T): UseState<T> => {
	const previousKeyRef = useRef(key);

	const [value, setValue] = useState<T>(() => {
		const storedValue = window.localStorage.getItem(key);
		if (storedValue === null) return initialValue;

		try {
			return JSON.parse(storedValue) as T;
		} catch {
			return initialValue;
		}
	});

	useEffect(() => {
		const previousKey = previousKeyRef.current;
		if (previousKey !== key) {
			window.localStorage.removeItem(previousKey);
			previousKeyRef.current = key;
		}
	}, [key]);

	useEffect(() => {
		const serializedValue = JSON.stringify(value);
		const serializedInitialValue = JSON.stringify(initialValue);

		if (serializedValue === serializedInitialValue) {
			window.localStorage.removeItem(key);
			return;
		}

		window.localStorage.setItem(key, serializedValue);
	}, [initialValue, key, value]);

	return [value, setValue];
};
