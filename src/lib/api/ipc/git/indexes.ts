import { appWindow } from '@tauri-apps/api/window';

export const subscribe = (
	params: { projectId: string },
	callback: (params: { projectId: string }) => Promise<void>
) => appWindow.listen(`project://${params.projectId}/git/index`, () => callback({ ...params }));
