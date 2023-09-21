import { invoke, listen } from '$lib/ipc';

export async function list(params: { projectId: string; sessionId: string; paths?: string[] }) {
	return invoke<Partial<Record<string, string>>>('list_session_files', params);
}

export function subscribe(
	params: { projectId: string; sessionId: string },
	callback: (params: { filePath: string; contents: string }) => Promise<void> | void
) {
	return listen<{ contents: string; filePath: string }>(
		`project://${params.projectId}/sessions/${params.sessionId}/files`,
		(event) => callback({ ...params, ...event.payload })
	);
}
