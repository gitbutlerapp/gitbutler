import { listen } from '$lib/ipc';

export function subscribe(
	params: { projectId: string },
	callback: (params: { projectId: string }) => Promise<void>
) {
	return listen(`project://${params.projectId}/git/index`, () => callback({ ...params }));
}
