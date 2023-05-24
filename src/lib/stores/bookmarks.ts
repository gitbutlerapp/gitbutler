import { writable, type Loadable, derived } from 'svelte-loadable-store';
import { bookmarks, type Bookmark } from '$lib/api';
import { get as getValue, type Readable } from '@square/svelte-store';

const stores: Record<string, Readable<Loadable<Bookmark[]>>> = {};

export const list = (params: { projectId: string }) => {
	if (params.projectId in stores) return stores[params.projectId];

	const store = writable(bookmarks.list(params), (set) =>
		bookmarks.subscribe(params, (bookmark) => {
			const oldValue = getValue(store);
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

export const get = (params: { projectId: string; timestampMs: number }) =>
	derived(list({ projectId: params.projectId }), (bookmarks) =>
		bookmarks.find((b) => b.timestampMs === params.timestampMs)
	);
