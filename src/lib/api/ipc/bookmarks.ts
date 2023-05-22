import { invoke, listen } from '$lib/ipc';

export type Bookmark = {
	projectId: string;
	timestampMs: number;
	note: string;
	deleted: boolean;
};

export const upsert = (params: {
	projectId: string;
	note: string;
	timestampMs: number;
	deleted: boolean;
}) => invoke<void>('upsert_bookmark', params);

export const list = (params: {
	projectId: string;
	range?: {
		start: number;
		end: number;
	};
}) => invoke<Bookmark[]>('list_bookmarks', params);

export const subscribe = (
	params: { projectId: string; range?: { start: number; end: number } },
	callback: (bookmark: Bookmark) => Promise<void> | void
) =>
	listen<Bookmark>(`project://${params.projectId}/bookmarks`, (event) => {
		if (
			params.range &&
			(event.payload.timestampMs < params.range.start ||
				event.payload.timestampMs >= params.range.end)
		)
			return;
		callback({ ...params, ...event.payload });
	});
