import { listen } from '$lib/backend/ipc';

export function subscribeToFetches(projectId: string, callback: () => Promise<void> | void) {
	return listen<any>(`project://${projectId}/git/fetch`, callback);
}
