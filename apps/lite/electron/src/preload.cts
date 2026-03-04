import { contextBridge, ipcRenderer } from "electron";
import type { LiteElectronApi } from "#electron/ipc";
import type { ProjectForFrontend, RefInfo } from "@gitbutler/but-sdk";

const api: LiteElectronApi = {
	async getVersion(): Promise<string> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("lite:get-version");
	},
	async headInfo(projectId: string): Promise<RefInfo> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("workspace:head-info", projectId);
	},
	async listProjects(): Promise<Array<ProjectForFrontend>> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("projects:list");
	},
	async ping(input: string): Promise<string> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("lite:ping", input);
	},
};

contextBridge.exposeInMainWorld("lite", api);
