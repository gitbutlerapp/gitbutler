import { writable, type Loadable } from 'svelte-loadable-store';
import { files } from '$lib/api';
import { get, type Readable } from '@square/svelte-store';

const stores: Record<string, Readable<Loadable<Record<string, string>>>> = {};

export default (params: { projectId: string; sessionId: string }) => {
	const key = `${params.projectId}/${params.sessionId}`;
	if (key in stores) return stores[key];

	const store = writable(files.list(params), (set) =>
		files.subscribe(params, ({ filePath, contents }) => {
			const oldValue = get(store);
			if (oldValue.isLoading) {
				files.list(params).then(set);
			} else {
				set({
					...oldValue.value,
					[filePath]: contents
				});
			}
		})
	);
	stores[key] = store;
	return store as Readable<Loadable<Record<string, string>>>;
};
