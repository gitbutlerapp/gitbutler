import { invoke } from '@tauri-apps/api';

export type SearchResult = {
    projectId: string;
    sessionId: string;
    filePath: string;
    // index of the delta in the session.
    index: number;
    highlighted: string[]; // contains the highlighted text
};

export const search = (params: {
    projectId: string;
    query: string;
    limit?: number;
    offset?: number;
}) => invoke<SearchResult[]>('search', params);
