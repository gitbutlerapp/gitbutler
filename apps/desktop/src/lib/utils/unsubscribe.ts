type MaybePromise<T> = T | Promise<T> | undefined;

export function unsubscribe(...unsubscribers: MaybePromise<() => any>[]) {
	return async () => {
		const awaitedUnsubscribers = await Promise.all(
			unsubscribers.map(async (u) => (u instanceof Promise ? await u : await Promise.resolve(u)))
		);

		const promises = awaitedUnsubscribers.map((unsubscriber) => unsubscriber?.());

		return await Promise.all(promises);
	};
}
