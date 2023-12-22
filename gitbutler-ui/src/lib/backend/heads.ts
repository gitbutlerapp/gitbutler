import { invoke, listen } from '$lib/backend/ipc';

export async function getHead(projectId: string) {
	const head = await invoke<string>('git_head', { projectId });
	return head.replace('refs/heads/', '');
}

export function subscribeToHead(projectId: string, callback: (head: string) => void) {
	return listen<{ head: string }>(`project://${projectId}/git/head`, (event) =>
		callback(event.payload.head.replace('refs/heads/', ''))
	);
}
