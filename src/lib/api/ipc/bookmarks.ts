import { invoke } from '$lib/ipc';

export type Bookmark = {
	id: string;
	projectId: string;
	createdTimestampMs: number;
	updatedTimestampMs: number;
	note: string;
	deleted: boolean;
};

export const upsert = (params: { bookmark: Bookmark }) => invoke<void>('upsert_bookmark', params);
