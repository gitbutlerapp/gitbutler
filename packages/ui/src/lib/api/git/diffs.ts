import { invoke } from '$lib/ipc';
import { asyncWritable, type WritableLoadable } from '@square/svelte-store';
import * as activities from './activities';
import * as sessions from '../ipc/sessions';

type Diffs = Partial<Record<string, string>>;

const list = (params: { projectId: string; contextLines?: number }): Promise<Diffs> =>
	invoke('git_wd_diff', {
		projectId: params.projectId,
		contextLines: params.contextLines ?? 10000
	});

const stores: Partial<Record<string, WritableLoadable<Diffs>>> = {};

export function getDiffsStore(params: { projectId: string }) {
	if (stores[params.projectId]) return stores[params.projectId];
	const store = asyncWritable([], () => list(params));
	activities.subscribe(params, ({ projectId }) => list({ projectId }).then(store.set));
	sessions.subscribe(params, () => list(params).then(store.set));
	stores[params.projectId] = store;
	return store;
}
