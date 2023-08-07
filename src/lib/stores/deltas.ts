import { writable, Loaded, Loadable } from 'svelte-loadable-store';
import * as deltas from '$lib/api/ipc/deltas';
import { get, type Writable } from '@square/svelte-store';
import type { Delta } from '$lib/api/ipc/deltas';

export interface DeltasStore extends Writable<Loadable<Record<string, Delta[]>>> {
	subscribeStream(params: { projectId: string; sessionId: string }): () => void;
}

export function getDeltasStore(params: { projectId: string; sessionId: string }) {
	const store = getDeltasStore2();
	store.subscribeStream(params);
	return store;
}
export function getDeltasStore2(): DeltasStore {
	const store = writable<Record<string, Delta[]>>();
	const subscribe = (innerParams: { projectId: string; sessionId: string }) => {
		console.log('subscribing streams', innerParams);
		deltas.list(innerParams).then((results) => {
			if (Loaded.isValue(results)) {
				store.set(results);
			}
		});
		return deltas.subscribe(innerParams, ({ filePath, deltas: newDeltas }) => {
			const oldValue = get(store);
			if (oldValue.isLoading) {
				deltas.list(innerParams).then((result) => {
					if (Loaded.isValue(result)) store.set(result);
				});
			} else if (Loaded.isError(oldValue)) {
				deltas.list(innerParams).then((result) => {
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
