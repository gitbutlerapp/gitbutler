import { invoke } from '$lib/ipc';

export type SearchResult = {
	projectId: string;
	sessionId: string;
	filePath: string;
	// index of the delta in the session.
	index: number;
};

export function list(params: {
	projectId: string;
	query: string;
	limit?: number;
	offset?: number;
}) {
	return invoke<{ total: number; page: SearchResult[] }>('search', params);
}
