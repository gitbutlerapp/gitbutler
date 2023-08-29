import { Session, list, subscribe } from '$lib/api/ipc/sessions';
import { asyncWritable, get, type Loadable, type WritableLoadable } from '@square/svelte-store';

export interface SessionsStore extends Loadable<Session[]> {
	subscribeStream(): () => void; // Consumer of store shall manage hsubscription
}

export function getSessionStore(params: { projectId: string }) {
	return getSessionStore2(params.projectId);
}

export function getSessionStore2(projectId: string): SessionsStore {
	const store = asyncWritable(
		[],
		async () => {
			const sessions = await list({ projectId: projectId });
            console.log('sessions', sessions);
			sessions.sort((a, b) => a.meta.startTimestampMs - b.meta.startTimestampMs);
			return sessions;
		},
		async (data) => data,
		{ trackState: true }
	) as WritableLoadable<Session[]>;
	const subscribeStream = () => {
		return subscribe({ projectId }, ({ session }) => {
			const oldValue = get(store);
			store.set(
				oldValue
					.filter((b) => b.id !== session.id)
					.concat({
						projectId,
						...session
					})
			);
		});
	};
	return { ...store, subscribeStream };
}
