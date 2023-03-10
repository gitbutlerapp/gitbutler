import { invoke } from '@tauri-apps/api';
import { appWindow } from '@tauri-apps/api/window';
import { writable, type Readable } from 'svelte/store';
import { log } from '$lib';
import type { Session } from '$lib/sessions';

export type Status = {
	path: string;
	status: string;
};

const listFiles = (params: { projectId: string }) =>
	invoke<Record<string, string>>('git_status', params);

function convertToStatuses(statusesGit: Record<string, string>): Status[] {
	return Object.entries(statusesGit).map((status) => {
		return {
			path: status[0],
			status: status[1]
		};
	});
}

export default async (params: { projectId: string }) => {
	const statusesGit = await listFiles(params);
	const statuses = convertToStatuses(statusesGit);

	const store = writable(statuses);

	appWindow.listen<Session>(`project://${params.projectId}/sessions`, async (event) => {
		log.info(`Status: Received sessions event, projectId: ${params.projectId}`);
		const statusesGit = await listFiles(params);
		const statuses = convertToStatuses(statusesGit);
		store.set(statuses);
	});

	return store as Readable<Status[]>;
};
