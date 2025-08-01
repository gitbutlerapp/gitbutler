import { contextBridge, ipcRenderer } from 'electron';

contextBridge.exposeInMainWorld('electronAPI', {
	openDirectory: async () => await ipcRenderer.invoke('dialog:openDirectory')
});
