type MaybePromise<T> = T | Promise<T> | undefined;

export function unsubscribe(...unsubscribers: MaybePromise<() => any>[]) {
	return async () => {
		const awaitedUnsubscribers = await Promise.all(unsubscribers);

		const promises = awaitedUnsubscribers.map((unsubscriber) => unsubscriber?.());

		return await Promise.all(promises);
	};
}
