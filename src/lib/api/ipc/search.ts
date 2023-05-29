import { invoke } from '$lib/ipc';

export type SearchResult = {
	projectId: string;
	sessionId: string;
	filePath: string;
	// index of the delta in the session.
	index: number;
};

export const list = (params: {
	projectId: string;
	query: string;
	limit?: number;
	offset?: number;
}) => invoke<{ total: number; page: SearchResult[] }>('search', params);
