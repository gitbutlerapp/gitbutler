export * as statuses from './statuses';
export { Status } from './statuses';
export * as activities from './activities';
export type { Activity } from './activities';
export * as heads from './heads';
export * as diffs from './diffs';
export * as indexes from './indexes';

import { invoke } from '$lib/ipc';

export const commit = (params: { projectId: string; message: string; push: boolean }) =>
	invoke<boolean>('git_commit', params);

export const stage = (params: { projectId: string; paths: Array<string> }) =>
	invoke<void>('git_stage', params);

export const unstage = (params: { projectId: string; paths: Array<string> }) =>
	invoke<void>('git_unstage', params);

export const matchFiles = (params: { projectId: string; matchPattern: string }) =>
	invoke<string[]>('git_match_paths', params);
