import { invoke } from '@tauri-apps/api';

export type SearchResult = {
    project_id: string;
    session_id: string;
    file_id: string;
    // index of the delta in the session.
    index: number;
};

export const search = (params: { project_id: string; query: string }) =>
    invoke<SearchResult>('search', params);
