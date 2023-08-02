export type { Activity } from './activities';

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
