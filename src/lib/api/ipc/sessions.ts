import { invoke, listen } from '$lib/ipc';
import { asyncWritable, type WritableLoadable } from '@square/svelte-store';

export namespace Session {
	export const within = (session: Session | undefined, timestampMs: number) => {
		if (!session) return false;
		const { startTimestampMs, lastTimestampMs } = session.meta;
		return startTimestampMs <= timestampMs && timestampMs <= lastTimestampMs;
	};
}

export type Session = {
	id: string;
	projectId: string;
	hash?: string;
	meta: {
		startTimestampMs: number;
		lastTimestampMs: number;
		branch?: string;
		commit?: string;
	};
};

export const list = async (params: { projectId: string; earliestTimestampMs?: number }) =>
	invoke<Omit<Session, 'projectId'>[]>('list_sessions', params).then((sessions) =>
		sessions.map((s) => ({ ...s, projectId: params.projectId }))
	);

export const subscribe = (
	params: { projectId: string },
	callback: (params: {
		projectId: string;
		session: Omit<Session, 'projectId'>;
	}) => Promise<void> | void
) =>
	listen<Omit<Session, 'projectId'>>(`project://${params.projectId}/sessions`, async (event) =>
		callback({ ...params, session: event.payload })
	);

const stores: Record<string, WritableLoadable<Session[]>> = {};

export const Sessions = (params: { projectId: string }) => {
	if (params.projectId in stores) return stores[params.projectId];
	const store = asyncWritable([], () => list(params));

	subscribe(params, ({ session }) => {
		store.update((sessions) => {
			const index = sessions.findIndex((s) => s.id === session.id);
			if (index === -1) return [...sessions, { ...session, projectId: params.projectId }];
			sessions[index] = { ...session, projectId: params.projectId };
			return sessions;
		});
	});
	stores[params.projectId] = store;
	return store;
};
