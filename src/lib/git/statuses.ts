import { invoke } from '@tauri-apps/api';
import { appWindow } from '@tauri-apps/api/window';
import { writable, type Readable } from 'svelte/store';

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

const list = (params: { projectId: string }) =>
	invoke<Record<string, Status>>('git_status', params);

export default async (params: { projectId: string }) => {
	const statuses = await list(params);
	const store = writable(statuses);

	[
		`project://${params.projectId}/git/index`,
		`project://${params.projectId}/git/activity`,
		`project://${params.projectId}/sessions`
	].forEach((eventName) => {
		appWindow.listen(eventName, async () => {
			store.set(await list(params));
		});
	});

	return store as Readable<Record<string, Status>>;
};
