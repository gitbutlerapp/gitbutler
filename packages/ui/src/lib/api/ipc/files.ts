import { invoke, listen } from '$lib/ipc';

export type FileContent = { type: 'utf8'; value: string } | { type: 'binary' } | { type: 'large' };

export async function list(params: { projectId: string; sessionId: string; paths?: string[] }) {
	return invoke<Partial<Record<string, FileContent>>>('list_session_files', params);
}

export function subscribe(
	params: { projectId: string; sessionId: string },
	callback: (params: { filePath: string; contents: FileContent | null }) => Promise<void> | void
) {
	return listen<{ contents: FileContent | null; filePath: string }>(
		`project://${params.projectId}/sessions/${params.sessionId}/files`,
		(event) => callback({ ...params, ...event.payload })
	);
}
