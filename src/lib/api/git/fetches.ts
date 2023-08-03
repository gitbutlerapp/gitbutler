import { listen } from '$lib/ipc';

export function subscribe(
	params: { projectId: string },
	callback: (params: { projectId: string }) => Promise<void> | void
) {
	return listen<{ head: string }>(`project://${params.projectId}/git/fetch`, (event) =>
		callback({ ...params, ...event.payload })
	);
}
