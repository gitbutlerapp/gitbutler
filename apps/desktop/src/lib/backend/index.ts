import {
	tauriInvoke,
	tauriListen,
	tauriCheck,
	tauriGetVersion,
	tauriReadFile
} from '$lib/backend/tauri';
import {
	webCheckUpdate,
	webCurrentVersion,
	webInvoke,
	webListen,
	webReadFile
} from '$lib/backend/web';

type Event<T> = {
	/** Event name */
	event: string;
	/** Event identifier used to unlisten */
	id: number;
	/** Event payload */
	payload: T;
};

export type DownloadEvent =
	| {
			event: 'Started';
			data: {
				contentLength?: number;
			};
	  }
	| {
			event: 'Progress';
			data: {
				chunkLength: number;
			};
	  }
	| {
			event: 'Finished';
	  };

export type DownloadEventName = DownloadEvent['event'];

export type DownloadUpdate = (onEvent?: (progress: DownloadEvent) => void) => Promise<void>;
export type InstallUpdate = () => Promise<void>;

export type Update = {
	version: string;
	currentVersion: string;
	body?: string;
	download: DownloadUpdate;
	install: InstallUpdate;
};

export interface IBackend {
	invoke: <T>(command: string, ...args: any[]) => Promise<T>;
	listen: <T>(event: string, callback: (event: Event<T>) => void) => () => Promise<void>;
	checkUpdate: () => Promise<Update | null>;
	currentVersion: () => Promise<string>;
	readFile: (path: string) => Promise<Uint8Array>;
}

class Tauri implements IBackend {
	invoke = tauriInvoke;
	listen = tauriListen;
	checkUpdate = tauriCheck;
	currentVersion = tauriGetVersion;
	readFile = tauriReadFile;
}

class Web implements IBackend {
	invoke = webInvoke;
	listen = webListen;
	checkUpdate = webCheckUpdate;
	currentVersion = webCurrentVersion;
	readFile = webReadFile;
}

export default function createBackend(): IBackend {
	if (import.meta.env.VITE_BUILD_TARGET === 'web') {
		return new Web();
	}
	return new Tauri();
}
