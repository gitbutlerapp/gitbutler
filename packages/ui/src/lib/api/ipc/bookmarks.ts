import { invoke, listen } from '$lib/ipc';

export type Bookmark = {
	projectId: string;
	timestampMs: number;
	note: string;
	deleted: boolean;
	updatedTimestampMs: number;
	createdTimestampMs: number;
};

export function upsert(params: {
	projectId: string;
	note: string;
	timestampMs: number;
	deleted: boolean;
}) {
	return invoke<void>('upsert_bookmark', params);
}

export function list(params: {
	projectId: string;
	range?: {
		start: number;
		end: number;
	};
}) {
	return invoke<Bookmark[]>('list_bookmarks', params);
}

export function subscribe(
	params: { projectId: string; range?: { start: number; end: number } },
	callback: (bookmark: Bookmark) => Promise<void> | void
) {
	return listen<Bookmark>(`project://${params.projectId}/bookmarks`, (event) => {
		if (
			params.range &&
			(event.payload.timestampMs < params.range.start ||
				event.payload.timestampMs >= params.range.end)
		)
			return;
		callback({ ...params, ...event.payload });
	});
}
