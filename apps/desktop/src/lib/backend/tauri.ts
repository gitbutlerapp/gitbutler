import { invoke as invokeIpc, listen as listenIpc } from './ipc';
import { getVersion } from '@tauri-apps/api/app';
import { checkUpdate, onUpdaterEvent } from '@tauri-apps/api/updater';

export class Tauri {
	invoke = invokeIpc;
	listen = listenIpc;
	checkUpdate = checkUpdate;
	onUpdaterEvent = onUpdaterEvent;
	currentVersion = getVersion;
}
