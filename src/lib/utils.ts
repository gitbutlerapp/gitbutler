export const debounce = <T extends (...args: any[]) => any>(fn: T, delay: number) => {
	let timeout: ReturnType<typeof setTimeout>;
	return (...args: any[]) => {
		clearTimeout(timeout);
		timeout = setTimeout(() => fn(...args), delay);
	};
};

export const clone = <T>(obj: T): T => structuredClone(obj);

type MaybePromise<T> = T | Promise<T>;

export const unsubscribe =
	(...unsubscribers: MaybePromise<() => void>[]) =>
	() =>
		unsubscribers.forEach((unsubscriber) => Promise.resolve(unsubscriber).then((fn) => fn()));
