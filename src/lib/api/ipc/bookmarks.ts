import { invoke } from '$lib/ipc';
import { nanoid } from 'nanoid';

export type Bookmark = {
	id: string;
	projectId: string;
	createdTimestampMs: number;
	updatedTimestampMs: number;
	note: string;
	deleted: boolean;
};

const newBookmark = (params: { projectId: string; note: string }) => ({
	id: nanoid(),
	projectId: params.projectId,
	createdTimestampMs: Date.now(),
	updatedTimestampMs: Date.now(),
	note: params.note,
	deleted: false
});

export { newBookmark as new };

export const upsert = (params: { bookmark: Bookmark }) => invoke<void>('upsert_bookmark', params);

export const list = (params: {
	projectId: string;
	range?: {
		start: number;
		end: number;
	};
}) => invoke<Bookmark[]>('list_bookmarks', params);
