export * as statuses from './statuses';
export { Status } from './statuses';
export * as activities from './activities';
export type { Activity } from './activities';
export * as heads from './heads';
export * as diffs from './diffs';
export * as indexes from './indexes';

import { invoke } from '$lib/ipc';

export function commit(params: { projectId: string; message: string; push: boolean }) {
	return invoke<boolean>('git_commit', params);
}

export function stage(params: { projectId: string; paths: Array<string> }) {
	return invoke<void>('git_stage', params);
}

export function unstage(params: { projectId: string; paths: Array<string> }) {
	return invoke<void>('git_unstage', params);
}

export function matchFiles(params: { projectId: string; matchPattern: string }) {
	return invoke<string[]>('git_match_paths', params);
}
