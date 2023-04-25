import { invoke } from '@tauri-apps/api';
import { writable } from 'svelte/store';
import { sessions, git } from '$lib/api';

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

export const Statuses = async (params: { projectId: string }) => {
	const store = writable(await list(params));
	sessions.subscribe(params, () => list(params).then(store.set));
	git.activities.subscribe(params, () => list(params).then(store.set));
	git.indexes.subscribe(params, () => list(params).then(store.set));
	return { subscribe: store.subscribe };
};
