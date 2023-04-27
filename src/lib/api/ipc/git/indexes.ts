import { listen } from '$lib/ipc';

export const subscribe = (
	params: { projectId: string },
	callback: (params: { projectId: string }) => Promise<void>
) => listen(`project://${params.projectId}/git/index`, () => callback({ ...params }));
