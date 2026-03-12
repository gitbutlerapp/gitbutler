import {
	type LiteElectronApi,
	type LongRunningTaskSnapshot,
} from "#electron/ipc";
import { contextBridge, ipcRenderer, type IpcRendererEvent } from "electron";
import type { ProjectForFrontend, RefInfo } from "@gitbutler/but-sdk";

const api: LiteElectronApi = {
	async listProjects(): Promise<ProjectForFrontend[]> {
		return await ipcRenderer.invoke("projects:list");
	},
	async headInfo(projectId: string): Promise<RefInfo> {
		return await ipcRenderer.invoke("workspace:head-info", projectId);
	},
	async listLongRunningTasks(): Promise<LongRunningTaskSnapshot[]> {
		return await ipcRenderer.invoke("long-running:list");
	},
	async startLongRunningTask(durationMs: number): Promise<number> {
		return await ipcRenderer.invoke("long-running:start", durationMs);
	},
	async cancelLongRunningTask(taskId: number): Promise<boolean> {
		return await ipcRenderer.invoke("long-running:cancel", taskId);
	},
	onLongRunningTaskEvent(listener: (event: LongRunningTaskSnapshot) => void): () => void {
		function eventListener(_event: IpcRendererEvent, taskEvent: LongRunningTaskSnapshot): void {
			listener(taskEvent);
		}

		ipcRenderer.on("long-running:event", eventListener);

		return () => {
			ipcRenderer.removeListener("long-running:event", eventListener);
		};
	},
};

contextBridge.exposeInMainWorld("lite", api);
