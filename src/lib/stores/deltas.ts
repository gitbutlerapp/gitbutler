import { writable, Loaded, Loadable } from 'svelte-loadable-store';
import * as deltas from '$lib/api/ipc/deltas';
import { get, type Writable } from '@square/svelte-store';
import type { Delta } from '$lib/api/ipc/deltas';

export interface DeltasStore extends Writable<Loadable<Record<string, Delta[]>>> {
	subscribeStream(sessionId: string): () => void;
}

export function getDeltasStore(params: { projectId: string; sessionId: string }) {
	const store = getDeltasStore2(params.projectId);
	store.subscribeStream(params.sessionId);
	return store;
}
export function getDeltasStore2(projectId: string): DeltasStore {
	const store = writable<Record<string, Delta[]>>();
	const subscribe = (sessionId: string) => {
		const combinedParams = { sessionId, projectId };
		deltas.list(combinedParams).then((results) => {
			if (Loaded.isValue(results)) {
				store.set(results);
			}
		});
		return deltas.subscribe(combinedParams, ({ filePath, deltas: newDeltas }) => {
			const oldValue = get(store);
			if (oldValue.isLoading) {
				deltas.list(combinedParams).then((result) => {
					if (Loaded.isValue(result)) store.set(result);
				});
			} else if (Loaded.isError(oldValue)) {
				deltas.list(combinedParams).then((result) => {
					if (Loaded.isValue(result)) store.set(result);
				});
			} else {
				oldValue;
				store.set({
					isLoading: false,
					value: {
						...oldValue.value,
						[filePath]: oldValue.value[filePath]
							? [...oldValue.value[filePath], ...newDeltas]
							: newDeltas
					}
				});
			}
		});
	};
	return { ...store, subscribeStream: subscribe };
}
