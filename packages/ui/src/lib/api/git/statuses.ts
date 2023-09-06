import { invoke } from '$lib/ipc';
import { asyncWritable, type WritableLoadable } from '@square/svelte-store';
import * as indexes from './indexes';
import * as activities from './activities';
import * as sessions from '../ipc/sessions';

type FileStatus = 'added' | 'modified' | 'deleted' | 'renamed' | 'typeChange' | 'other';

export type Status =
	| { staged: FileStatus }
	| { unstaged: FileStatus }
	| { staged: FileStatus; unstaged: FileStatus };

export function isStaged(status: Status): status is { staged: FileStatus } {
	return 'staged' in status && status.staged !== null;
}
export function isUnstaged(status: Status): status is { unstaged: FileStatus } {
	return 'unstaged' in status && status.unstaged !== null;
}

export function list(params: { projectId: string }) {
	return invoke<Record<string, Status>>('git_status', params);
}

const stores: Record<string, WritableLoadable<Record<string, Status>>> = {};

export function getStatusStore(params: { projectId: string }) {
	if (stores[params.projectId]) return stores[params.projectId];
	const store = asyncWritable([], () => list(params));
	sessions.subscribe(params, () => list(params).then(store.set));
	activities.subscribe(params, () => list(params).then(store.set));
	indexes.subscribe(params, () => list(params).then(store.set));
	stores[params.projectId] = store;
	return store;
}
