import { invoke as invokeIpc, listen as listenIpc } from '$lib/backend/ipc';
import { getVersion } from '@tauri-apps/api/app';
import { check } from '@tauri-apps/plugin-updater';

export class Tauri {
	invoke = invokeIpc;
	listen = listenIpc;
	checkUpdate = check;
	currentVersion = getVersion;
}
