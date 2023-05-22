import { asyncWritable, type Loadable } from '@square/svelte-store';
import { type Bookmark, bookmarks } from '$lib/api';

export type Store = Loadable<Bookmark[]> & {
	create: (params?: { note?: string; timestampMs?: number }) => Promise<Bookmark>;
};

const stores: Record<string, Store> = {};

export default (params: { projectId: string }): Store => {
	const { projectId } = params;
	if (projectId in stores) {
		return stores[projectId];
	}
	const store = asyncWritable<[], Bookmark[]>(
		[],
		() => bookmarks.list(params),
		async (newValue, _parents, oldValue) => {
			const changedBookmarks = newValue.filter((bookmark) => {
				const oldBookmark = oldValue?.find((b) => b.timestampMs === bookmark.timestampMs);
				if (!oldBookmark) return true;
				return oldBookmark !== bookmark;
			});
			await Promise.all(changedBookmarks.map((bookmark) => bookmarks.upsert(bookmark)));
			return newValue;
		}
	);
	return {
		...store,
		create: async ({ timestampMs, note }: { note?: string; timestampMs?: number } = {}) => {
			const newBookmark = {
				projectId,
				timestampMs: timestampMs ?? Date.now(),
				note: note ?? '',
				deleted: false
			};
			await store.update((value) => [...value, newBookmark]);
			return newBookmark;
		}
	};
};
