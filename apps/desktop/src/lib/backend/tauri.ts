import { invoke as invokeIpc, listen as listenIpc } from './ipc';
import { getVersion } from '@tauri-apps/api/app';
import { check } from '@tauri-apps/plugin-updater';

export class Tauri {
	invoke = invokeIpc;
	listen = listenIpc;
	checkUpdate = import.meta.env.FLATPAK_ID ? () => null : check;
	currentVersion = getVersion;
}
