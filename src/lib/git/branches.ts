import { invoke } from '@tauri-apps/api';
import { appWindow } from '@tauri-apps/api/window';
import { writable, type Readable } from 'svelte/store';
import { log } from '$lib';

export type Branch = {
	oid: string;
	branch: string;
	name: string;
	description: string;
	lastCommitTs: number;
	firstCommitTs: number;
	ahead: number;
	behind: number;
	upstream: string;
	authors: string[];
};

const list = (params: { projectId: string }) =>
	invoke<Branch[]>('git_branches', params);

export default async (params: { projectId: string }) => {
	const branches = await list(params);
	const store = writable(branches);

	console.log('branches', branches);

	appWindow.listen(`project://${params.projectId}/git/activity`, async () => {
		log.info(`Status: Received git activity event, projectId: ${params.projectId}`);
		const newBranches = await list({ projectId: params.projectId });
		store.set(newBranches);
	});

	return store as Readable<Branch[]>;
};
