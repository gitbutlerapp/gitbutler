import { writable, type Loadable, derived, Value } from 'svelte-loadable-store';
import { bookmarks, type Bookmark } from '$lib/api';
import { get as getValue, type Readable } from '@square/svelte-store';

const stores: Record<string, Readable<Loadable<Bookmark[]>>> = {};

export function list(params: { projectId: string }) {
	if (params.projectId in stores) return stores[params.projectId];

	const store = writable(bookmarks.list(params), (set) => {
		const unsubscribe = bookmarks.subscribe(params, (bookmark) => {
			const oldValue = getValue(store);
			if (oldValue.isLoading) {
				bookmarks.list(params).then(set);
			} else if (Value.isError(oldValue.value)) {
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
	return store as Readable<Loadable<Bookmark[]>>;
}

export function get(params: { projectId: string; timestampMs: number }) {
	return derived(list({ projectId: params.projectId }), (bookmarks) =>
		bookmarks.find((b) => b.timestampMs === params.timestampMs)
	);
}
