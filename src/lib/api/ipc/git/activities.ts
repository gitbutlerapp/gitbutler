import { invoke, listen } from '$lib/ipc';
import { asyncWritable, get, type WritableLoadable } from '@square/svelte-store';

export type Activity = {
	type: string;
	timestampMs: number;
	message: string;
};

export function list(params: { projectId: string; startTimeMs?: number }) {
	return invoke<Activity[]>('git_activity', params);
}

export function subscribe(
	params: { projectId: string },
	callback: (params: { projectId: string }) => Promise<void> | void
) {
	return listen(`project://${params.projectId}/git/activity`, () => callback(params));
}

const stores: Record<string, WritableLoadable<Activity[]>> = {};

export function Activities(params: { projectId: string }) {
	if (stores[params.projectId]) return stores[params.projectId];

	const store = asyncWritable([], () => list(params));
	subscribe(params, async () => {
		const activity = get(store);
		const startTimeMs = activity.at(-1)?.timestampMs;
		const newActivities = await list({ projectId: params.projectId, startTimeMs });
		store.update((activities) => [...activities, ...newActivities]);
	});
	stores[params.projectId] = store;
	return store;
}
