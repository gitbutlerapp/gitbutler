import type { Readable } from 'svelte/store';
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
	systemTheme: Readable<string | null>;
	invoke: <T>(command: string, ...args: any[]) => Promise<T>;
	listen: <T>(event: string, callback: (event: Event<T>) => void) => () => Promise<void>;
	checkUpdate: () => Promise<Update | null>;
	currentVersion: () => Promise<string>;
	readFile: (path: string) => Promise<Uint8Array>;
	openExternalUrl: (href: string) => Promise<void>;
	relaunch: () => Promise<void>;
}
