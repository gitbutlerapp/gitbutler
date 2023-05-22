export * as git from './git';
export { Status, type Activity } from './git';
export * as deltas from './deltas';
export { type Delta, Operation } from './deltas';
export * as sessions from './sessions';
export { Session } from './sessions';
export * as users from './users';
export * as projects from './projects';
export type { Project } from './projects';
export * as searchResults from './search';
export { type SearchResult } from './search';
export * as files from './files';
export * as zip from './zip';
export * as bookmarks from './bookmarks';
export type { Bookmark } from './bookmarks';

import { invoke } from '$lib/ipc';

export const deleteAllData = () => invoke<void>('delete_all_data');
