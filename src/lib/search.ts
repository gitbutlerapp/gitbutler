import { invoke } from '@tauri-apps/api';

export type SearchResult = {
    projectId: string;
    sessionId: string;
    filePath: string;
    // index of the delta in the session.
    index: number;
    // contains the highlighted text snippets.
    highlighted: string[];
};

export const search = (params: {
    projectId: string;
    query: string;
    limit?: number;
    offset?: number;
}) => invoke<{ total: number; page: SearchResult[] }>('search', params);
