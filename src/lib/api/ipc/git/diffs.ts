import { invoke } from '$lib/ipc';
import { asyncWritable, type WritableLoadable } from '@square/svelte-store';
import { sessions, git } from '$lib/api';

const list = (params: { projectId: string; contextLines?: number }) =>
	invoke<Record<string, string>>('git_wd_diff', {
		projectId: params.projectId,
		contextLines: params.contextLines ?? 10000
	});

const stores: Record<string, WritableLoadable<Record<string, string>>> = {};

export function Diffs(params: { projectId: string }) {
	if (stores[params.projectId]) return stores[params.projectId];
	const store = asyncWritable([], () => list(params));
	git.activities.subscribe(params, ({ projectId }) => list({ projectId }).then(store.set));
	sessions.subscribe(params, () => list(params).then(store.set));
	stores[params.projectId] = store;
	return store;
}
