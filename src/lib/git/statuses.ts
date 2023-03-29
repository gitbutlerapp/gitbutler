import { invoke } from '@tauri-apps/api';
import { appWindow } from '@tauri-apps/api/window';
import { writable, type Readable } from 'svelte/store';
import { log } from '$lib';

export type Status = {
	path: string;
	status: FileStatus;
};

type FileStatus = 'added' | 'modified' | 'deleted' | 'renamed' | 'typeChange' | 'other';

const list = (params: { projectId: string }) =>
	invoke<Record<string, FileStatus>>('git_status', params);

const convertToStatuses = (statusesGit: Record<string, FileStatus>): Status[] =>
	Object.entries(statusesGit).map((status) => ({
		path: status[0],
		status: status[1]
	}));

export default async (params: { projectId: string }) => {
	const statuses = await list(params).then(convertToStatuses);
	const store = writable(statuses);

	appWindow.listen(`project://${params.projectId}/git/index`, async () => {
		log.info(`Status: Received git index event, projectId: ${params.projectId}`);
		store.set(await list(params).then(convertToStatuses));
	});

	appWindow.listen(`project://${params.projectId}/sessions`, async () => {
		log.info(`Status: Received sessions event, projectId: ${params.projectId}`);
		store.set(await list(params).then(convertToStatuses));
	});

	return store as Readable<Status[]>;
};
