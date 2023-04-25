import { invoke } from '@tauri-apps/api';
import { appWindow } from '@tauri-apps/api/window';
import { get, writable } from 'svelte/store';

export type Activity = {
	type: string;
	timestampMs: number;
	message: string;
};

export const list = (params: { projectId: string; startTimeMs?: number }) =>
	invoke<Activity[]>('git_activity', params);

export const subscribe = (
	params: { projectId: string },
	callback: (params: { projectId: string }) => Promise<void> | void
) => appWindow.listen(`project://${params.projectId}/git/activity`, () => callback(params));

export const Activities = async (params: { projectId: string }) => {
	const store = writable(await list(params));
	subscribe(params, async () => {
		const activity = get(store);
		const startTimeMs = activity.at(-1)?.timestampMs;
		const newActivities = await list({ projectId: params.projectId, startTimeMs });
		store.update((activities) => [...activities, ...newActivities]);
	});
	return { subscribe: store.subscribe };
};
