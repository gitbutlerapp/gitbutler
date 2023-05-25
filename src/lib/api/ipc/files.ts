import { invoke, listen } from '$lib/ipc';

export const list = async (params: { projectId: string; sessionId: string; paths?: string[] }) =>
	invoke<Record<string, string>>('list_session_files', params);

export const subscribe = (
	params: { projectId: string; sessionId: string },
	callback: (params: { filePath: string; contents: string }) => Promise<void> | void
) =>
	listen<{ contents: string; filePath: string }>(
		`project://${params.projectId}/sessions/${params.sessionId}/files`,
		(event) => callback({ ...params, ...event.payload })
	);
