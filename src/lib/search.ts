import { invoke } from '@tauri-apps/api';

export type SearchResult = {
	projectId: string;
	sessionId: string;
	filePath: string;
	// index of the delta in the session.
	index: number;
};

export const search = (params: { projectId: string; query: string }) =>
	invoke<SearchResult[]>('search', params);
