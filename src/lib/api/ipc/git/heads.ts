import { invoke, listen } from '$lib/ipc';
import { asyncWritable, type WritableLoadable } from '@square/svelte-store';

export const get = (params: { projectId: string }) => invoke<string>('git_head', params);

export const subscribe = (
	params: { projectId: string },
	callback: (params: { projectId: string; head: string }) => Promise<void> | void
) =>
	listen<{ head: string }>(`project://${params.projectId}/git/head`, (event) =>
		callback({ ...params, ...event.payload })
	);

const stores: Record<string, WritableLoadable<string>> = {};

export const Head = (params: { projectId: string }) => {
	if (stores[params.projectId]) return stores[params.projectId];
	const store = asyncWritable([], () =>
		get(params).then((head) => head.replace('refs/heads/', ''))
	);
	subscribe(params, ({ head }) => store.set(head.replace('refs/heads/', '')));
	stores[params.projectId] = store;
	return store;
};
