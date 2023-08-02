import { invoke, listen } from '$lib/ipc';

export type OperationDelete = { delete: [number, number] };
export type OperationInsert = { insert: [number, string] };

export type Operation = OperationDelete | OperationInsert;

export function isDelete(operation: Operation): operation is OperationDelete {
	return 'delete' in operation;
}

export function isInsert(operation: Operation): operation is OperationInsert {
	return 'insert' in operation;
}

export type Delta = { timestampMs: number; operations: Operation[] };

export async function list(params: { projectId: string; sessionId: string; paths?: string[] }) {
	return invoke<Record<string, Delta[]>>('list_deltas', params);
}

export function subscribe(
	params: { projectId: string; sessionId: string },
	callback: (params: {
		projectId: string;
		sessionId: string;
		filePath: string;
		deltas: Delta[];
	}) => Promise<void> | void
) {
	return listen<{ deltas: Delta[]; filePath: string }>(
		`project://${params.projectId}/sessions/${params.sessionId}/deltas`,
		(event) => callback({ ...params, ...event.payload })
	);
}
