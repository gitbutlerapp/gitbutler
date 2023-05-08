import { invoke } from '$lib/ipc';
import { asyncWritable, type WritableLoadable } from '@square/svelte-store';
import * as indexes from './indexes';
import * as activities from './activities';
import * as sessions from '../sessions';

type FileStatus = 'added' | 'modified' | 'deleted' | 'renamed' | 'typeChange' | 'other';

export type Status =
	| { staged: FileStatus }
	| { unstaged: FileStatus }
	| { staged: FileStatus; unstaged: FileStatus };

export namespace Status {
	export const isStaged = (status: Status): status is { staged: FileStatus } =>
		'staged' in status && status.staged !== null;
	export const isUnstaged = (status: Status): status is { unstaged: FileStatus } =>
		'unstaged' in status && status.unstaged !== null;
}

export const list = (params: { projectId: string }) =>
	invoke<Record<string, Status>>('git_status', params);

const stores: Record<string, WritableLoadable<Record<string, Status>>> = {};

export const Statuses = (params: { projectId: string }) => {
	if (stores[params.projectId]) return stores[params.projectId];
	const store = asyncWritable([], () => list(params));
	sessions.subscribe(params, () => list(params).then(store.set));
	activities.subscribe(params, () => list(params).then(store.set));
	indexes.subscribe(params, () => list(params).then(store.set));
	stores[params.projectId] = store;
	return store;
};
