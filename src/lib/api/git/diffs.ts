import { invoke } from '$lib/ipc';
import { asyncWritable, type WritableLoadable } from '@square/svelte-store';
import * as activities from './activities';
import * as sessions from '../ipc/sessions';

const list = (params: { projectId: string; contextLines?: number }) =>
	invoke<Record<string, string>>('git_wd_diff', {
		projectId: params.projectId,
		contextLines: params.contextLines ?? 10000
	});

const stores: Record<string, WritableLoadable<Record<string, string>>> = {};

export function getCommitDiff(params: { projectId: string; commitId: string }) {
	return invoke<Record<string, string>>('git_commit_diff', params);
}

export function getDiffsStore(params: { projectId: string }) {
	if (stores[params.projectId]) return stores[params.projectId];
	const store = asyncWritable([], () => list(params));
	activities.subscribe(params, ({ projectId }) => list({ projectId }).then(store.set));
	sessions.subscribe(params, () => list(params).then(store.set));
	stores[params.projectId] = store;
	return store;
}
