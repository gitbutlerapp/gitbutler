import { writable, type Loadable, Loaded } from 'svelte-loadable-store';
import * as files from '$lib/api/ipc/files';
import { get, type Readable } from '@square/svelte-store';

type Files = Partial<Record<string, string>>;

const stores: Partial<Record<string, Readable<Loadable<Files>>>> = {};

export function getFilesStore(params: {
	projectId: string;
	sessionId: string;
}): Readable<Loadable<Files>> {
	const key = `${params.projectId}/${params.sessionId}`;
	const cached = stores[key];
	if (cached) return cached;

	const store = writable(files.list(params), (set) => {
		const unsubscribe = files.subscribe(params, ({ filePath, contents }) => {
			const oldValue = get(store);
			if (oldValue.isLoading) {
				files.list(params).then(set);
			} else if (Loaded.isError(oldValue)) {
				files.list(params).then(set);
			} else {
				set({
					...oldValue.value,
					[filePath]: contents
				});
			}
		});
		return () => {
			Promise.resolve(unsubscribe).then((unsubscribe) => unsubscribe());
		};
	});
	stores[key] = store;
	return store as Readable<Loadable<Files>>;
}
