import { contextBridge, ipcRenderer } from 'electron';
import type { LiteElectronApi } from '#electron/ipc';
import type { ProjectForFrontend } from '@gitbutler/but-sdk';

const api: LiteElectronApi = {
	async ping(input: string): Promise<string> {
		return await ipcRenderer.invoke('lite:ping', input);
	},
	async getVersion(): Promise<string> {
		return await ipcRenderer.invoke('lite:get-version');
	},
	async listProjects(): Promise<ProjectForFrontend[]> {
		return await ipcRenderer.invoke('projects:list');
	}
};

contextBridge.exposeInMainWorld('lite', api);
