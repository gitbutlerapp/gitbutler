import { Session, list, subscribe } from '$lib/api/ipc/sessions';
import type { WritableReloadable } from '$lib/vbranches/types';
import { asyncWritable, get } from '@square/svelte-store';

export function getSessionStore(params: { projectId: string }) {
	const { store, unsubscribe } = getSessionStore2(params);
	return store;
}

export function getSessionStore2(params: { projectId: string }) {
	console.log('getting sessions!');
	const store = asyncWritable(
		[],
		async () => {
			const sessions = await list(params);
			sessions.sort((a, b) => a.meta.startTimestampMs - b.meta.startTimestampMs);
			return sessions;
		},
		async (data) => data,
		{ trackState: true }
	) as WritableReloadable<Session[]>;
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
	return { unsubscribe, store };
}
