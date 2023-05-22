import { invoke } from '$lib/ipc';

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
