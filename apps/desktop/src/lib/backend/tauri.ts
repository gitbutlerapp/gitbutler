import { invoke as invokeIpc, listen as listenIpc } from '$lib/backend/ipc';
import { getVersion } from '@tauri-apps/api/app';
import { check } from '@tauri-apps/plugin-updater';
import { getCurrentWindow } from '@tauri-apps/api/window';

export class Tauri {
	invoke = invokeIpc;
	listen = listenIpc;
	checkUpdate = check;
	currentVersion = getVersion;

	private window = getCurrentWindow();

	async minimize() {
		await this.window.minimize();
	}

	async toggleMaximize() {
		const isMaximized = await this.window.isMaximized();
		if (isMaximized) {
			await this.window.unmaximize();
		} else {
			await this.window.maximize();
		}
	}

	async close() {
		await this.window.close();
	}

	async setDecorations(decorations: boolean) {
		await this.window.setDecorations(decorations);
	}
}
