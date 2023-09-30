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

export async function list(projectId: string) {
	return invoke<Omit<Session, 'projectId'>[]>('list_sessions', { projectId }).then((sessions) =>
		sessions.map((s) => ({ ...s, projectId: projectId }))
	);
}

export function subscribe(
	projectId: string,
	callback: (session: Omit<Session, 'projectId'>) => void
) {
	return listen<Omit<Session, 'projectId'>>(`project://${projectId}/sessions`, async (event) =>
		callback(event.payload)
	);
}
