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

type DialogFilter = {
	/** Filter name. */
	name: string;
	/**
	 * Extensions to filter, without a `.` prefix.
	 * @example
	 * ```typescript
	 * extensions: ['svg', 'png']
	 * ```
	 */
	extensions: string[];
};

export type OpenDialogOptions = {
	/** The title of the dialog window (desktop only). */
	title?: string;
	/** The filters of the dialog. */
	filters?: DialogFilter[];
	/**
	 * Initial directory or file path.
	 * If it's a directory path, the dialog interface will change to that folder.
	 * If it's not an existing directory, the file name will be set to the dialog's file name input and the dialog will be set to the parent folder.
	 *
	 * On mobile the file name is always used on the dialog's file name input.
	 * If not provided, Android uses `(invalid).txt` as default file name.
	 */
	defaultPath?: string;
	/** Whether the dialog allows multiple selection or not. */
	multiple?: boolean;
	/** Whether the dialog is a directory selection or not. */
	directory?: boolean;
	/**
	 * If `directory` is true, indicates that it will be read recursively later.
	 * Defines whether subdirectories will be allowed on the scope or not.
	 */
	recursive?: boolean;
	/** Whether to allow creating directories in the dialog. Enabled by default. **macOS Only** */
	canCreateDirectories?: boolean;
};

export type OpenDialogReturn<T extends OpenDialogOptions> = T['directory'] extends true
	? T['multiple'] extends true
		? string[] | null
		: string | null
	: T['multiple'] extends true
		? string[] | null
		: string | null;

export type AppInfo = {
	name: string;
	version: string;
};
export interface IBackend {
	platformName: string;
	systemTheme: Readable<string | null>;
	invoke: <T>(command: string, ...args: any[]) => Promise<T>;
	listen: <T>(event: string, callback: (event: Event<T>) => void) => () => Promise<void>;
	checkUpdate: () => Promise<Update | null>;
	currentVersion: () => Promise<string>;
	readFile: (path: string) => Promise<Uint8Array>;
	openExternalUrl: (href: string) => Promise<void>;
	relaunch: () => Promise<void>;
	documentDir: () => Promise<string>;
	joinPath: (path: string, ...paths: string[]) => Promise<string>;
	filePicker<T extends OpenDialogOptions>(options: T): Promise<OpenDialogReturn<T>>;
	homeDirectory(): Promise<string>;
	getAppInfo: () => Promise<AppInfo>;
	writeTextToClipboard: (text: string) => Promise<void>;
	readTextFromClipboard: () => Promise<string>;
}
