import { invoke } from '$lib/ipc';

export function matchFiles(params: { projectId: string; matchPattern: string }) {
	return invoke<string[]>('git_match_paths', params);
}
