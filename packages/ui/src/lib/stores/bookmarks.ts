import { writable, type Loadable, derived, Loaded } from 'svelte-loadable-store';
import * as bookmarks from '$lib/api/ipc/bookmarks';
import { get as getValue, type Readable } from '@square/svelte-store';

const stores: Record<string, Readable<Loadable<bookmarks.Bookmark[]>>> = {};

export function getBookmarksStore(params: { projectId: string }) {
	if (params.projectId in stores) return stores[params.projectId];

	const store = writable(bookmarks.list(params), (set) => {
		const unsubscribe = bookmarks.subscribe(params, (bookmark) => {
			const oldValue = getValue(store);
			if (oldValue.isLoading) {
				bookmarks.list(params).then(set);
			} else if (Loaded.isError(oldValue)) {
				bookmarks.list(params).then(set);
			} else {
				set(oldValue.value.filter((b) => b.timestampMs !== bookmark.timestampMs).concat(bookmark));
			}
		});
		return () => {
			Promise.resolve(unsubscribe).then((unsubscribe) => unsubscribe());
		};
	});
	stores[params.projectId] = store;
	return store as Readable<Loadable<bookmarks.Bookmark[]>>;
}

export function getBookmark(params: { projectId: string; timestampMs: number }) {
	return derived(getBookmarksStore({ projectId: params.projectId }), (bookmarks) =>
		bookmarks.find((b) => b.timestampMs === params.timestampMs)
	);
}
