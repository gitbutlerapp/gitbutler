import { listen } from '$lib/backend/ipc';

export function subscribe(
	params: { projectId: string },
	callback: (params: { projectId: string }) => Promise<void>
) {
	return listen(`project://${params.projectId}/git/index`, async () => callback({ ...params }));
}
