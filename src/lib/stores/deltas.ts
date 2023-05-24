import { writable, type Loadable, Value } from 'svelte-loadable-store';
import { deltas, type Delta } from '$lib/api';
import { get, type Readable } from '@square/svelte-store';

const stores: Record<string, Readable<Loadable<Record<string, Delta[]>>>> = {};

export default (params: { projectId: string; sessionId: string }) => {
	const key = `${params.projectId}/${params.sessionId}`;
	if (key in stores) return stores[key];

	const store = writable(deltas.list(params), (set) => {
		const unsubscribe = deltas.subscribe(params, ({ filePath, deltas: newDeltas }) => {
			const oldValue = get(store);
			if (oldValue.isLoading) {
				deltas.list(params).then(set);
			} else if (Value.isError(oldValue.value)) {
				deltas.list(params).then(set);
			} else {
				set({
					...oldValue.value,
					[filePath]: oldValue.value[filePath]
						? [...oldValue.value[filePath], ...newDeltas]
						: newDeltas
				});
			}
		});
		return () => {
			Promise.resolve(unsubscribe).then((unsubscribe) => unsubscribe());
		};
	});
	stores[key] = store;
	return store as Readable<Loadable<Record<string, Delta[]>>>;
};
