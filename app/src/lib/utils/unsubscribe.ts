type MaybePromise<T> = T | Promise<T> | undefined;

export function unsubscribe(...unsubscribers: MaybePromise<() => any>[]) {
	return async () => {
		const promises = unsubscribers.map(async (unsubscriber) => (await unsubscriber)?.());

		return await Promise.all(promises);
	};
}
