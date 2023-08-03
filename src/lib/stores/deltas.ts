import { writable, Loaded } from 'svelte-loadable-store';
import * as deltas from '$lib/api/ipc/deltas';
import { get } from '@square/svelte-store';

export function getDeltasStore(params: { projectId: string; sessionId: string }) {
	const { store } = getDeltasStore2(params);
	return store;
}
export function getDeltasStore2(params: { projectId: string; sessionId: string }) {
	const store = writable(deltas.list(params));
	const unsubscribe = deltas.subscribe(params, ({ filePath, deltas: newDeltas }) => {
		const oldValue = get(store);
		if (oldValue.isLoading) {
			deltas.list(params).then((result) => {
				if (Loaded.isValue(result)) store.set(result);
			});
		} else if (Loaded.isError(oldValue)) {
			deltas.list(params).then((result) => {
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
	return { store, unsubscribe };
}
