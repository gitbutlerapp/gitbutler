import { invoke } from '@tauri-apps/api';
import { appWindow } from '@tauri-apps/api/window';
import { derived, writable } from 'svelte/store';
import { log } from '$lib';

const list = (params: { projectId: string }) => invoke<string>('git_head', params);

export default async (params: { projectId: string }) => {
	const head = await list(params);
	const store = writable(head);

	appWindow.listen<{ head: string }>(`project://${params.projectId}/git/head`, async (payload) => {
		log.info(`Status: Received git head event, projectId: ${params.projectId}`);
		store.set(payload.payload.head);
	});

	return derived(store, (head) => head.replace('refs/heads/', ''));
};
