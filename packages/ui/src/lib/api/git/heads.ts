import { invoke, listen } from '$lib/ipc';
import { asyncWritable, type WritableLoadable } from '@square/svelte-store';

export async function getHead(projectId: string) {
	const head = await invoke<string>('git_head', { projectId });
	return head.replace('refs/heads/', '');
}

export function subscribe(projectId: string, callback: (head: string) => Promise<void> | void) {
	return listen<{ head: string }>(`project://${projectId}/git/head`, (event) =>
		callback(event.payload.head.replace('refs/heads/', ''))
	);
}

export function getHeadStore(projectId: string): WritableLoadable<string> {
	return asyncWritable(
		[],
		async () => await getHead(projectId),
		undefined,
		undefined,
		(set) => {
			const unsubscribe = subscribe(projectId, set);
			return () => unsubscribe();
		}
	);
}
