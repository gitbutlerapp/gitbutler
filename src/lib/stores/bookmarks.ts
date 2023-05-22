import { asyncWritable, type Loadable } from '@square/svelte-store';
import { type Bookmark, bookmarks } from '$lib/api';

export type Store = Loadable<Bookmark[]> & {
	create: (params: { note: string }) => Promise<Bookmark>;
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
			const changedBookmarks = newValue.filter((bookmark, index) => {
				const oldBookmark = oldValue?.[index];
				if (!oldBookmark) return true;
				return bookmark.updatedTimestampMs !== oldBookmark.updatedTimestampMs;
			});
			await Promise.all(changedBookmarks.map((bookmark) => bookmarks.upsert({ bookmark })));
			return newValue;
		}
	);
	return {
		...store,
		create: async (params: { note: string } = { note: '' }) => {
			const newBookmark = bookmarks.new({ ...params, projectId });
			await store.update((value) => [...value, newBookmark]);
			return newBookmark;
		}
	};
};
