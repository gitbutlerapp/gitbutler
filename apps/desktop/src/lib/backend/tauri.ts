import { invoke as invokeIpc, listen as listenIpc } from '$lib/backend/ipc';
import { getVersion } from '@tauri-apps/api/app';
import { check } from '@tauri-apps/plugin-updater';
import { getCurrentWindow } from '@tauri-apps/api/window';

export class Tauri {
	invoke = invokeIpc;
	listen = listenIpc;
	checkUpdate = check;
	currentVersion = getVersion;

	// Window control methods
	async minimize() {
		const window = getCurrentWindow();
		await window.minimize();
	}

	async toggleMaximize() {
		const window = getCurrentWindow();
		const isMaximized = await window.isMaximized();
		if (isMaximized) {
			await window.unmaximize();
		} else {
			await window.maximize();
		}
	}

	async close() {
		const window = getCurrentWindow();
		await window.close();
	}

	async setDecorations(decorations: boolean) {
		const window = getCurrentWindow();
		await window.setDecorations(decorations);
	}
}
