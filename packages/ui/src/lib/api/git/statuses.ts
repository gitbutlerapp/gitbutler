import { invoke } from '$lib/ipc';
import { asyncWritable, type WritableLoadable } from '@square/svelte-store';
import * as indexes from './indexes';
import * as activities from './activities';
import { subscribeToSessions } from '../ipc/sessions';

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

export function list(params: { projectId: string }): Promise<Statuses> {
	return invoke('git_status', params);
}

type Statuses = Partial<Record<string, Status>>;

const stores: Partial<Record<string, WritableLoadable<Statuses>>> = {};

export function getStatusStore(params: { projectId: string }): WritableLoadable<Statuses> {
	const cached = stores[params.projectId];
	if (cached) return cached;
	const store = asyncWritable([], () => list(params));
	subscribeToSessions(params.projectId, () => list(params).then(store.set));
	activities.subscribe(params, () => list(params).then(store.set));
	indexes.subscribe(params, () => list(params).then(store.set));
	stores[params.projectId] = store;
	return store;
}
