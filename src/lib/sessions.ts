import { invoke } from '@tauri-apps/api';
import { appWindow } from '@tauri-apps/api/window';
import { writable, type Readable } from 'svelte/store';
import { log } from '$lib';

export type Activity = {
	type: string;
	timestampMs: number;
	message: string;
};

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
	activity: Activity[];
};

export const listFiles = async (params: {
	projectId: string;
	sessionId: string;
	paths?: string[];
}) => {
	const start = performance.now();
	const result = await invoke<Record<string, string>>('list_session_files', params);
	log.debug(`list_session_files took ${performance.now() - start}ms`);
	return result;
};

const list = async (params: { projectId: string }) => {
	const start = performance.now();
	const result = await invoke<Session[]>('list_sessions', params);
	log.debug(`list_sessions took ${performance.now() - start}ms`);
	return result;
};

export default async (params: { projectId: string; earliestTimestampMs?: number }) => {
	const store = writable([] as Session[]);
	list(params).then((sessions) => {
		store.set(sessions);
	});

	appWindow.listen<Session>(`project://${params.projectId}/sessions`, async (event) => {
		log.info(`Received sessions event, projectId: ${params.projectId}`);
		const session = event.payload;
		store.update((sessions) => {
			const index = sessions.findIndex((session) => session.id === event.payload.id);
			if (index === -1) {
				return [...sessions, session];
			} else {
				return [...sessions.slice(0, index), session, ...sessions.slice(index + 1)];
			}
		});
	});

	return store as Readable<Session[]>;
};
