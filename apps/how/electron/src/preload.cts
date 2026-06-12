import { contextBridge, ipcRenderer } from "electron";
import type { HowElectronApi, HowStatus, OpenProjectResult, StatusEvent } from "./ipc";

const api: HowElectronApi = {
	getStatus: async () => await (ipcRenderer.invoke("how:get-status") as Promise<HowStatus>),
	openProject: async () => await (ipcRenderer.invoke("how:open-project") as Promise<OpenProjectResult>),
	startProject: async () => await (ipcRenderer.invoke("how:start-project") as Promise<OpenProjectResult>),
	createCheckpointNow: async () => await (ipcRenderer.invoke("how:create-checkpoint-now") as Promise<HowStatus>),
	onStatus: (callback) => {
		function listener(_event: Electron.IpcRendererEvent, status: StatusEvent) {
			callback(status);
		}
		ipcRenderer.on("how:status", listener);
		return () => ipcRenderer.removeListener("how:status", listener);
	},
	platform: process.platform,
};

contextBridge.exposeInMainWorld("how", api);
