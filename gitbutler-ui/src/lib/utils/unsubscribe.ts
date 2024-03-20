type MaybePromise<T> = T | Promise<T>;

export function unsubscribe(...unsubscribers: MaybePromise<() => void>[]) {
	return () => {
		unsubscribers.forEach((unsubscriber) => Promise.resolve(unsubscriber).then((fn) => fn()));
	};
}
