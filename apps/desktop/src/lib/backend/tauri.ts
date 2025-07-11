import { invoke as invokeIpc, listen as listenIpc } from '$lib/backend/ipc';
import { getVersion } from '@tauri-apps/api/app';
import { check } from '@tauri-apps/plugin-updater';

export const IS_TAURI_ENV = '__TAURI_INTERNALS__' in window;

export class Tauri {
	invoke = invokeIpc;
	listen = listenIpc;
	checkUpdate = check;
	currentVersion = getVersion;
}
