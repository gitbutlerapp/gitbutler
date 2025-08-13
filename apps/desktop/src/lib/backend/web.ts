import { isReduxError } from '$lib/state/reduxError';
import { getCookie } from '$lib/utils/cookies';
import { readable } from 'svelte/store';
import type { IBackend, OpenDialogOptions, OpenDialogReturn } from '$lib/backend/backend';
import path from 'path';

export default class Web implements IBackend {
	systemTheme = readable<string | null>(null);
	invoke = webInvoke;
	listen = webListen;
	checkUpdate = webCheckUpdate;
	currentVersion = webCurrentVersion;
	readFile = webReadFile;
	openExternalUrl = webOpenExternalUrl;
	relaunch = webRelaunch;
	documentDir = webDocumentDir;
	joinPath = webJoinPath;
	filePicker<T extends OpenDialogOptions>(options?: T): Promise<OpenDialogReturn<T>> {
		return webFilePicker<T>(options);
	}
}

function webJoinPath(pathSegment: string, ...paths: string[]): Promise<string> {
	// TODO: We might want to expose some endpoint in the backedn to handle path joining in the right way.
	// This will break on windows
	return Promise.resolve(path.join(pathSegment, ...paths));
}

function webDocumentDir(): Promise<string> {
	// This needs to be implemented for the web version
	return Promise.resolve('');
}

function webFilePicker<T extends OpenDialogOptions>(options?: T): Promise<OpenDialogReturn<T>> {
	const fileInput = document.createElement('input');
	fileInput.type = 'file';

	if (options?.multiple) {
		fileInput.multiple = true;
	}

	const promise = new Promise<OpenDialogReturn<T>>((resolve) => {
		fileInput.onchange = () => {
			const files = fileInput.files;
			if (!files) {
				resolve(null);
				return;
			}
			const paths: string[] = [];
			for (const file of files) {
				paths.push(file.name);
			}
			if (paths.length === 0) {
				resolve(null);
				return;
			}
			if (options?.multiple) {
				resolve(paths as OpenDialogReturn<T>);
				return;
			}

			const path = paths[0];
			resolve(path as OpenDialogReturn<T>);
		};
	});

	fileInput.click();
	return promise;
}

function webRelaunch(): Promise<void> {
	// The web version does not support relaunching
	throw new Error('Relaunch is not implemented in the web version');
}

/**
 * Invokes a backend web command via HTTP POST and returns the result.
 *
 * @template T The expected type of the response subject.
 * @param command - The name of the backend command to invoke.
 * @param params - An optional object containing parameters for the command.
 * @returns A promise that resolves with the subject of the response if successful.
 * @throws Throws an error if the backend responds with an error or if the request fails.
 */
async function webInvoke<T>(command: string, params: Record<string, unknown> = {}): Promise<T> {
	try {
		const response = await fetch(`http://${getWebUrl()}`, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({ command, params })
		});
		const out: ServerResonse<T> = await response.json();
		if (out.type === 'success') {
			return out.subject;
		} else {
			if (isReduxError(out.subject)) {
				console.error(`ipc->${command}: ${JSON.stringify(params)}`, out.subject);
			}
			throw out.subject;
		}
	} catch (error: unknown) {
		if (isReduxError(error)) {
			console.error(`ipc->${command}: ${JSON.stringify(params)}`, error);
		}
		throw error;
	}
}

/**
 * Registers an event listener for a specified event name using the singleton `WebListener` instance.
 *
 * @template T - The type of the event payload.
 * @param event - The name of the event to listen for.
 * @param handle - The callback function to handle the event when it is triggered.
 * @returns A function or object that can be used to remove or manage the event listener, as provided by `WebListener.listen`.
 */
function webListen<T>(event: EventName, handle: EventCallback<T>) {
	const webListener = WebListener.getInstance();
	return webListener.listen({ name: event, handle });
}

async function webCheckUpdate(): Promise<null> {
	// TODO: Implement this for the web version if needed
	return null;
}

async function webCurrentVersion(): Promise<string> {
	// TODO: Implement this for the web version if needed
	return '0.0.0';
}

async function webReadFile(_path: string): Promise<Uint8Array> {
	// TODO: Implement this for the web version if needed
	throw new Error('webReadFile is not implemented for the web version');
}

async function webOpenExternalUrl(href: string): Promise<void> {
	window.open(href, '_blank');
	return await Promise.resolve();
}

class WebListener {
	private socket: WebSocket | undefined;
	private count = 0;
	private handlers: { name: EventName; handle: EventCallback<any> }[] = [];
	private static instance: WebListener | undefined;

	private constructor() {}

	static getInstance(): WebListener {
		if (!WebListener.instance) {
			WebListener.instance = new WebListener();
		}
		return WebListener.instance;
	}

	listen(handler: { name: EventName; handle: EventCallback<any> }): () => Promise<void> {
		this.handlers.push(handler);
		this.count++;
		if (!this.socket) {
			this.socket = new WebSocket(`ws://${getWebUrl()}/ws`);
			this.socket.addEventListener('message', (event) => {
				const data: { name: string; payload: any } = JSON.parse(event.data);
				for (const handler of this.handlers) {
					if (handler.name === data.name) {
						// The id is an artifact from tauri, we don't use it so
						// I've used a random value
						handler.handle({ event: data.name, payload: data.payload, id: 69 });
					}
				}
			});
		}

		// This needs to be async just so it's the same API as the tauri version
		return async () => {
			this.handlers = this.handlers.filter((h) => h !== handler);
			this.count--;
			if (this.count === 0) {
				this.socket?.close();
				this.socket = undefined;
			}
		};
	}
}

function getWebUrl(): string {
	const host = getCookie('butlerHost') || import.meta.env.VITE_BUTLER_HOST || 'localhost';
	const port = getCookie('butlerPort') || import.meta.env.VITE_BUTLER_PORT || '6978';
	return `${host}:${port}`;
}

type EventName = string;

interface Event<T> {
	/** Event name */
	event: string;
	/** Event identifier used to unlisten */
	id: number;
	/** Event payload */
	payload: T;
}

type EventCallback<T> = (event: Event<T>) => void;

type ServerResonse<T> =
	| {
			type: 'success';
			subject: T;
	  }
	| {
			type: 'error';
			subject: unknown;
	  };
