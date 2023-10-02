import { Session, listSessions, subscribeToSessions } from '$lib/api/ipc/sessions';
import { asyncWritable, get, type Loadable, type WritableLoadable } from '@square/svelte-store';

export function getSessionStore(projectId: string): Loadable<Session[]> {
	const store = asyncWritable(
		[],
		async () => await listSessions(projectId),
		async (data) => data,
		{ trackState: true },
		(set) => {
			const unsubscribe = subscribeToSessions(projectId, (session) => {
				const oldValue = get(store)?.filter((b) => b.id != session.id);
				const newValue = {
					projectId,
					...session
				};
				// It's possible for a subscription event to happen before the store
				// has finished loading, and the store value is undefined until then.
				// TODO: But if that happens, the latest session gets overwritten?
				set(oldValue ? [newValue, ...oldValue] : [newValue]);
			});
			return () => unsubscribe();
		}
	) as WritableLoadable<Session[]>;
	return store;
}
