type MaybePromise<T> = T | Promise<T> | undefined;

export function unsubscribe(...unsubscribers: MaybePromise<() => any>[]) {
	return () => {
		const promises = unsubscribers.map(async (unsubscriber) => (await unsubscriber)?.());

		return Promise.all(promises);
	};
}
