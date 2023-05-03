import { invoke } from '$lib/ipc';
import { asyncWritable } from '@square/svelte-store';
import { sessions, git } from '$lib/api';

const list = (params: { projectId: string; contextLines?: number }) =>
	invoke<Record<string, string>>('git_wd_diff', {
		projectId: params.projectId,
		contextLines: params.contextLines ?? 10000
	});

export const Diffs = (params: { projectId: string }) => {
	const store = asyncWritable([], () => list(params));
	git.activities.subscribe(params, ({ projectId }) => list({ projectId }).then(store.set));
	sessions.subscribe(params, () => list(params).then(store.set));
	return store;
};
