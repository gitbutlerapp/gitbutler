import { invoke, listen } from '$lib/ipc';
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
) => listen(`project://${params.projectId}/git/activity`, () => callback(params));

export const Activities = async (params: { projectId: string }) => {
	const store = writable<Activity[]>(await list(params));
	subscribe(params, async () => {
		const activity = get(store);
		const startTimeMs = activity.at(-1)?.timestampMs;
		const newActivities = await list({ projectId: params.projectId, startTimeMs });
		store.update((activities) => [...activities, ...newActivities]);
	});
	return { subscribe: store.subscribe };
};
