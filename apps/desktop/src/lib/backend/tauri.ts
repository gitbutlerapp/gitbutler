import { isReduxError } from '$lib/state/reduxError';
import { getVersion as tauriGetVersion } from '@tauri-apps/api/app';
import { invoke as invokeTauri } from '@tauri-apps/api/core';
import { listen as listenTauri } from '@tauri-apps/api/event';
import { getCurrentWindow, Window } from '@tauri-apps/api/window';
import { readFile as tauriReadFile } from '@tauri-apps/plugin-fs';
import { check as tauriCheck } from '@tauri-apps/plugin-updater';
import { readable } from 'svelte/store';
import type { IBackend } from '$lib/backend/backend';
import type { EventCallback, EventName } from '@tauri-apps/api/event';

export default class Tauri implements IBackend {
	private appWindow: Window | undefined;
	systemTheme = readable<string | null>(null, (set) => {
		if (!this.appWindow) {
			this.appWindow = getCurrentWindow();
		}
		this.appWindow.theme().then((value) => {
			set(value);
		});

		this.appWindow.onThemeChanged((e) => {
			set(e.payload);
		});
	});
	invoke = tauriInvoke;
	listen = tauriListen;
	checkUpdate = tauriCheck;
	currentVersion = tauriGetVersion;
	readFile = tauriReadFile;
	openExternalUrl = tauriOpenExternalUrl;
}

async function tauriInvoke<T>(command: string, params: Record<string, unknown> = {}): Promise<T> {
	// This commented out code can be used to delay/reject an api call
	// return new Promise<T>((resolve, reject) => {
	// 	if (command.startsWith('apply')) {
	// 		setTimeout(() => {
	// 			reject('testing the error page');
	// 		}, 500);
	// 	} else {
	// 		resolve(invokeTauri<T>(command, params));
	// 	}
	// }).catch((reason) => {
	// 	const userError = UserError.fromError(reason);
	// 	console.error(`ipc->${command}: ${JSON.stringify(params)}`, userError);
	// 	throw userError;
	// });

	try {
		return await invokeTauri<T>(command, params);
	} catch (error: unknown) {
		if (isReduxError(error)) {
			console.error(`ipc->${command}: ${JSON.stringify(params)}`, error);
		}
		throw error;
	}
}

function tauriListen<T>(event: EventName, handle: EventCallback<T>) {
	const unlisten = listenTauri(event, handle);
	return async () => await unlisten.then((unlistenFn) => unlistenFn());
}

async function tauriOpenExternalUrl(href: string): Promise<void> {
	return await invokeTauri<void>('open_url', { url: href });
}
