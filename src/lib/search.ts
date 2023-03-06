import { invoke } from '@tauri-apps/api';

export type SearchResult = {
	projectId: string;
	sessionId: string;
	filePath: string;
	// index of the delta in the session.
	index: number;
	timestampMsGte?: number;
	timestampMsLt?: number;
};

export const search = (params: {
	projectId: string;
	query: string;
	limit?: number;
	offset?: number;
}) => invoke<SearchResult[]>('search', params);
