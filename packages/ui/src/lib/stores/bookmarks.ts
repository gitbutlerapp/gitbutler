import * as bookmarks from '$lib/api/ipc/bookmarks';
import { type Loadable, asyncWritable, asyncDerived } from '@square/svelte-store';

export function getBookmarksStore(params: { projectId: string }): Loadable<bookmarks.Bookmark[]> {
	return asyncWritable(
		[],
		async () => await bookmarks.list(params),
		undefined,
		{ trackState: true },
		(set, update) => {
			const unsubscribe = bookmarks.subscribe(params, (bookmark) => {
				update((oldValue) =>
					oldValue.filter((b) => b.timestampMs !== bookmark.timestampMs).concat(bookmark)
				);
			});
			return () => {
				Promise.resolve(unsubscribe).then((unsubscribe) => unsubscribe());
			};
		}
	);
}

export function getBookmark(params: { projectId: string; timestampMs: number }) {
	return asyncDerived(
		getBookmarksStore({ projectId: params.projectId }),
		async (bookmarks) => bookmarks.find((b) => b.timestampMs === params.timestampMs),
		{ trackState: true }
	);
}
