import { writable, type Loadable } from 'svelte-loadable-store';
import { bookmarks, type Bookmark } from '$lib/api';
import { get, type Readable } from 'svelte/store';

const stores: Record<string, Readable<Loadable<Bookmark[]>>> = {};

export default (params: { projectId: string }) => {
	if (params.projectId in stores) return stores[params.projectId];

	const store = writable(bookmarks.list(params), (set) =>
		bookmarks.subscribe(params, (bookmark) => {
			const oldValue = get(store);
			if (oldValue.isLoading) {
				bookmarks.list(params).then(set);
			} else {
				set(oldValue.value.filter((b) => b.timestampMs !== bookmark.timestampMs).concat(bookmark));
			}
		})
	);
	stores[params.projectId] = store;
	return store as Readable<Loadable<Bookmark[]>>;
};
