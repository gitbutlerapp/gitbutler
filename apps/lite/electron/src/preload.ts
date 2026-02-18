import { liteIpcChannels } from '#electron/ipc';
import { contextBridge, ipcRenderer } from 'electron';

import type { LiteElectronApi } from '#electron/ipc';

const api: LiteElectronApi = {
	async ping(input: string): Promise<string> {
		return await ipcRenderer.invoke(liteIpcChannels.ping, input);
	},
	async getVersion(): Promise<string> {
		return await ipcRenderer.invoke(liteIpcChannels.getVersion);
	}
};

contextBridge.exposeInMainWorld('lite', api);
