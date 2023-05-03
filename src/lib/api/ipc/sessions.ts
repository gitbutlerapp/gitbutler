import { invoke, listen } from '$lib/ipc';
import { clone } from '$lib/utils';
import { asyncWritable } from '@square/svelte-store';

export namespace Session {
	export const within = (session: Session | undefined, timestampMs: number) => {
		if (!session) return false;
		const { startTimestampMs, lastTimestampMs } = session.meta;
		return startTimestampMs <= timestampMs && timestampMs <= lastTimestampMs;
	};
}

export type Session = {
	id: string;
	hash?: string;
	meta: {
		startTimestampMs: number;
		lastTimestampMs: number;
		branch?: string;
		commit?: string;
	};
};
const cache: Record<string, Promise<Session[]>> = {};

export const list = async (params: { projectId: string; earliestTimestampMs?: number }) => {
	if (params.projectId in cache) {
		return cache[params.projectId].then((sessions) =>
			clone(sessions).filter((s) =>
				params.earliestTimestampMs ? s.meta.startTimestampMs >= params.earliestTimestampMs : true
			)
		);
	}
	cache[params.projectId] = invoke<Session[]>('list_sessions', {
		projectId: params.projectId
	});
	return cache[params.projectId].then((sessions) =>
		clone(sessions).filter((s) =>
			params.earliestTimestampMs ? s.meta.startTimestampMs >= params.earliestTimestampMs : true
		)
	);
};

export const subscribe = (
	params: { projectId: string },
	callback: (params: { projectId: string; session: Session }) => Promise<void> | void
) =>
	listen<Session>(`project://${params.projectId}/sessions`, async (event) =>
		callback({ ...params, session: event.payload })
	);

export const Sessions = (params: { projectId: string }) => {
	const store = asyncWritable([], () => list(params));
	subscribe(params, ({ session }) => {
		store.update((sessions) => {
			const index = sessions.findIndex((s) => s.id === session.id);
			if (index === -1) return [...sessions, session];
			sessions[index] = session;
			return sessions;
		});
	});
	return store;
};
