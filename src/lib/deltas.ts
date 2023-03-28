import { log } from '$lib';
import { invoke } from '@tauri-apps/api';
import { appWindow } from '@tauri-apps/api/window';
import { writable, type Readable } from 'svelte/store';

export type OperationDelete = { delete: [number, number] };
export type OperationInsert = { insert: [number, string] };

export type Operation = OperationDelete | OperationInsert;

export namespace Operation {
	export const isDelete = (operation: Operation): operation is OperationDelete =>
		'delete' in operation;

	export const isInsert = (operation: Operation): operation is OperationInsert =>
		'insert' in operation;
}

export type Delta = { timestampMs: number; operations: Operation[] };

export type DeltasEvent = {
	deltas: Delta[];
	filePath: string;
};

export const list = async (params: { projectId: string; sessionId: string; paths?: string[] }) =>
	invoke<Record<string, Delta[]>>('list_deltas', params);

export const subscribe = (
	params: { projectId: string; sessionId: string },
	callback: (filepath: string, deltas: Delta[]) => void
) => {
	log.info(`Subscribing to deltas for ${params.projectId}, ${params.sessionId}`);
	return appWindow.listen<DeltasEvent>(
		`project://${params.projectId}/sessions/${params.sessionId}/deltas`,
		(event) => {
			log.info(
				`Received deltas for ${params.projectId}, ${params.sessionId}, ${event.payload.filePath}`
			);
			callback(event.payload.filePath, event.payload.deltas);
		}
	);
};

export default async (params: { projectId: string; sessionId: string }) => {
	const init = await list(params);

	const store = writable<Record<string, Delta[]>>(init);
	subscribe(params, (filepath, newDeltas) =>
		store.update((deltas) => ({
			...deltas,
			[filepath]: newDeltas
		}))
	);

	return store as Readable<Record<string, Delta[]>>;
};
