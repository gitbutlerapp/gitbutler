import { listen } from '$lib/backend/ipc';

export function subscribe(projectId: string, callback: () => Promise<void> | void) {
	return listen<any>(`project://${projectId}/git/fetch`, callback);
}
