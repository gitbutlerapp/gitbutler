import { invoke, listen } from '$lib/ipc';

export namespace Session {
	export function within(session: Session | undefined, timestampMs: number) {
		if (!session) return false;
		const { startTimestampMs, lastTimestampMs } = session.meta;
		return startTimestampMs <= timestampMs && timestampMs <= lastTimestampMs;
	}
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

export async function list(params: { projectId: string; earliestTimestampMs?: number }) {
	return invoke<Omit<Session, 'projectId'>[]>('list_sessions', params).then((sessions) =>
		sessions.map((s) => ({ ...s, projectId: params.projectId }))
	);
}

export function subscribe(
	params: { projectId: string },
	callback: (params: {
		projectId: string;
		session: Omit<Session, 'projectId'>;
	}) => Promise<void> | void
) {
	return listen<Omit<Session, 'projectId'>>(
		`project://${params.projectId}/sessions`,
		async (event) => callback({ ...params, session: event.payload })
	);
}
