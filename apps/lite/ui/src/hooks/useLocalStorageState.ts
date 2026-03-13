import { Dispatch, SetStateAction, useEffect, useRef, useState } from "react";

export const useLocalStorageState = <T>(
	key: string,
	initialValue: T,
): [T, Dispatch<SetStateAction<T>>] => {
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
