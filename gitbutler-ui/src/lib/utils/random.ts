export function debounce<T extends (...args: any[]) => any>(fn: T, delay: number) {
	let timeout: ReturnType<typeof setTimeout>;
	return (...args: any[]) => {
		clearTimeout(timeout);
		timeout = setTimeout(() => fn(...args), delay);
	};
}

export function clone<T>(obj: T): T {
	return structuredClone(obj);
}

type MaybePromise<T> = T | Promise<T>;

export function unsubscribe(...unsubscribers: MaybePromise<() => void>[]) {
	return () => {
		unsubscribers.forEach((unsubscriber) => Promise.resolve(unsubscriber).then((fn) => fn()));
	};
}
