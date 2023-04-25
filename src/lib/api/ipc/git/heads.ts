import { invoke } from '@tauri-apps/api';
import { appWindow } from '@tauri-apps/api/window';
import { derived, writable } from 'svelte/store';

export const get = (params: { projectId: string }) => invoke<string>('git_head', params);

export const subscribe = (
	params: { projectId: string },
	callback: (params: { projectId: string; head: string }) => Promise<void> | void
) =>
	appWindow.listen<{ head: string }>(`project://${params.projectId}/git/head`, (event) =>
		callback({ ...params, ...event.payload })
	);

export const Head = async (params: { projectId: string }) => {
	const store = writable(await get(params));
	subscribe(params, ({ head }) => store.set(head));
	return derived(store, (head) => head.replace('refs/heads/', ''));
};
