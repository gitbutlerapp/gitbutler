import { invoke, listen } from '$lib/ipc';
import { clone } from '$lib/utils';
import { asyncWritable, type WritableLoadable } from '@square/svelte-store';

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

export const subscribe = (
	params: { projectId: string; sessionId: string },
	callback: (params: {
		projectId: string;
		sessionId: string;
		filePath: string;
		deltas: Delta[];
	}) => Promise<void> | void
) =>
	listen<{ deltas: Delta[]; filePath: string }>(
		`project://${params.projectId}/sessions/${params.sessionId}/deltas`,
		(event) => callback({ ...params, ...event.payload })
	);

const stores: Record<string, Record<string, WritableLoadable<Record<string, Delta[]>>>> = {};

export const Deltas = (params: { projectId: string; sessionId: string }) => {
	const projectStores = stores[params.projectId] || {};
	if (params.sessionId in projectStores) return projectStores[params.sessionId];

	const store = asyncWritable([], () => list(params));
	subscribe(params, ({ filePath, deltas }) => {
		store.update((deltasCache) => ({
			...deltasCache,
			[filePath]: deltas
		}));
	});
	projectStores[params.sessionId] = store;
	stores[params.projectId] = projectStores;
	return store;
};
