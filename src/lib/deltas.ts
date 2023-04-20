import { log } from '$lib';
import { invoke } from '@tauri-apps/api';
import { appWindow } from '@tauri-apps/api/window';
import { writable, type Readable } from 'svelte/store';
import { clone } from './utils';

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

const cache: Record<string, Record<string, Promise<Record<string, Delta[]>>>> = {};

export const list = async (params: { projectId: string; sessionId: string; paths?: string[] }) => {
	const sessionDeltasCache = cache[params.projectId] || {};
	if (params.sessionId in sessionDeltasCache) {
		return sessionDeltasCache[params.sessionId].then((deltas) =>
			Object.fromEntries(
				Object.entries(clone(deltas)).filter(([path]) =>
					params.paths ? params.paths.includes(path) : true
				)
			)
		);
	}

	const promise = invoke<Record<string, Delta[]>>('list_deltas', {
		sessionId: params.sessionId,
		projectId: params.projectId
	});
	sessionDeltasCache[params.sessionId] = promise;
	cache[params.projectId] = sessionDeltasCache;
	return promise.then((deltas) =>
		Object.fromEntries(
			Object.entries(clone(deltas)).filter(([path]) =>
				params.paths ? params.paths.includes(path) : true
			)
		)
	);
};

export default async (params: { projectId: string; sessionId: string }) => {
	const init = await list(params);

	const store = writable<Record<string, Delta[]>>(init);
	appWindow.listen<DeltasEvent>(
		`project://${params.projectId}/sessions/${params.sessionId}/deltas`,
		(event) => {
			log.info(
				`Received deltas for ${params.projectId}, ${params.sessionId}, ${event.payload.filePath}`
			);
			store.update((deltas) => ({
				...deltas,
				[event.payload.filePath]: event.payload.deltas
			}));
		}
	);

	return store as Readable<Record<string, Delta[]>>;
};
