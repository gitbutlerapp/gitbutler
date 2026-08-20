import { contextBridge, ipcRenderer } from "electron";
import { createLiteApi } from "./lite-api.js";

/**
 * The electron binding of the renderer's api: `createLiteApi` builds the
 * whole surface; this file only supplies the transport it runs over.
 */
const api = createLiteApi({
	// electron types `invoke` as `Promise<any>`; the annotation narrows it
	// to `unknown` so wire results are not silently `any` everywhere.
	invoke: (channel: string, ...args: Array<unknown>): Promise<unknown> =>
		ipcRenderer.invoke(channel, ...args),
	subscribe: (channel, listener) => {
		const ipcListener = (_event: Electron.IpcRendererEvent, payload: unknown) => {
			listener(payload);
		};
		ipcRenderer.on(channel, ipcListener);
		return () => ipcRenderer.removeListener(channel, ipcListener);
	},
	platform: process.platform,
});

contextBridge.exposeInMainWorld("lite", api);
