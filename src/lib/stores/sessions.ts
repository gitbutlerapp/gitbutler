import { Session, list, subscribe } from '$lib/api/ipc/sessions';
import { asyncWritable, get, type WritableLoadable } from '@square/svelte-store';

const stores: Record<string, WritableLoadable<Session[]>> = {};

export function getSessionStore(params: { projectId: string }) {
	if (params.projectId in stores) return stores[params.projectId];
	const store = asyncWritable(
		[],
		async () => {
			const sessions = await list(params);
			sessions.sort((a, b) => a.meta.startTimestampMs - b.meta.startTimestampMs);
			return sessions;
		},
		async (data) => data
	);
	// TODO: Where do we unsubscribe this?
	const unsubscribe = subscribe(params, ({ session }) => {
		const oldValue = get(store);
		store.set(
			oldValue
				.filter((b) => b.id !== session.id)
				.concat({
					projectId: params.projectId,
					...session
				})
		);
	});
	stores[params.projectId] = store;
	return store;
}
