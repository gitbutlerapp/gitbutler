import { Session, list, subscribe } from '$lib/api/ipc/sessions';
import { asyncWritable, get, type Loadable, type WritableLoadable } from '@square/svelte-store';

export function getSessionStore(projectId: string): Loadable<Session[]> {
	const store = asyncWritable(
		[],
		async () => await list(projectId),
		async (data) => data,
		{ trackState: true },
		(set) => {
			const unsubscribe = subscribe(projectId, (session) => {
				const oldValue = get(store).filter((b) => b.id != session.id);
				const newValue = {
					projectId,
					...session
				};
				set([newValue, ...oldValue]);
			});
			return () => {
				unsubscribe();
			};
		}
	) as WritableLoadable<Session[]>;
	return store;
}
