import type { Readable } from 'svelte/store';

export interface IBackend {
	/**
	 * The name of the platform, e.g. 'macos', 'windows', 'linux', or 'web'
	 */
	platformName: string;
	/**
	 * The theme of the system, e.g. 'light', 'dark'
	 */
	systemTheme: Readable<string | null>;
	/**
	 * Executes a command in the backend.
	 */
	invoke: <T>(command: string, ...args: any[]) => Promise<T>;
	/**
	 * Subscribes to an event in the backend.
	 */
	listen: <T>(event: string, callback: (event: Event<T>) => void) => () => Promise<void>;
	/**
	 * Checks for updates in the backend.
	 */
	checkUpdate: () => Promise<Update | null>;
	/**
	 * Returns the current version of the application.
	 */
	currentVersion: () => Promise<string>;
	/**
	 * Reads a file from the disk.
	 */
	readFile: (path: string) => Promise<Uint8Array>;
	/**
	 * Opens an external URL in the system's default browser.
	 */
	openExternalUrl: (href: string) => Promise<void>;
	/**
	 * Relaunches the application.
	 */
	relaunch: () => Promise<void>;
	/**
	 * Returns the absolute path to the user's document directory.
	 */
	documentDir: () => Promise<string>;
	/**
	 * Joins path segments into a single path, taking care of platform-specific path separators.
	 */
	joinPath: (path: string, ...paths: string[]) => Promise<string>;
	/**
	 * Opens a file picker dialog to select files or directories.
	 */
	filePicker<T extends OpenDialogOptions>(options: T): Promise<OpenDialogReturn<T>>;
	/**
	 * Returns the absolute path to the user's home directory.
	 */
	homeDirectory(): Promise<string>;
	/**
	 * Gets the application name and version.
	 */
	getAppInfo: () => Promise<AppInfo>;
	/**
	 * Writes text to the system clipboard.
	 */
	writeTextToClipboard: (text: string) => Promise<void>;
	/**
	 * Reads text from the system clipboard.
	 */
	readTextFromClipboard: () => Promise<string>;
	/**
	 * Loads a disk store from a file.
	 */
	loadDiskStore: (fileName: string) => Promise<DiskStore>;
	/**
	 * Sets the window title.
	 */
	setWindowTitle: (title: string) => void;
}

export interface DiskStore {
	set: (key: string, value: unknown) => Promise<void>;

	get<T>(key: string, defaultValue: undefined): Promise<T | undefined>;
	get<T>(key: string, defaultValue: T): Promise<T>;
	get<T>(key: string, defaultValue?: T): Promise<T | undefined>;
}

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
