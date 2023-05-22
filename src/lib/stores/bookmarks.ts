import { writable, type Loadable } from 'svelte-loadable-store';
import { bookmarks, type Bookmark } from '$lib/api';
import type { Readable } from 'svelte/store';

const stores: Record<string, Readable<Loadable<Bookmark[]>>> = {};

export default (params: { projectId: string }) => {
	if (params.projectId in stores) return stores[params.projectId];

	const { subscribe } = writable(bookmarks.list(params), (set) =>
		bookmarks.subscribe(params, () => bookmarks.list(params).then(set))
	);
	const store = { subscribe };
	stores[params.projectId] = store;
	return store;
};
