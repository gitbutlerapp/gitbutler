import { isReduxError } from "$lib/error/reduxError";
import { getCookie } from "$lib/utils/cookies";
import ReconnectingWebSocket from "reconnecting-websocket";
import { readable } from "svelte/store";
import type {
	AppInfo,
	DiskStore,
	IBackend,
	OpenDialogOptions,
	OpenDialogReturn,
} from "$lib/backend/backend";

export default class Web implements IBackend {
	platformName = "web";
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
	homeDirectory = webHomeDirectory;
	getAppInfo = webGetAppInfo;
	readTextFromClipboard = webReadTextFromClipboard;
	writeTextToClipboard = webWriteTextToClipboard;
	async filePicker<T extends OpenDialogOptions>(options?: T): Promise<OpenDialogReturn<T>> {
		return await webFilePicker<T>(options);
	}
	async loadDiskStore(_filename: string): Promise<DiskStore> {
		// For the web version, we don't have a disk store, so we return a no-op implementation
		return new WebDiskStore();
	}

	async getWindowTitle(): Promise<string> {
		return await Promise.resolve(document.title);
	}

	setWindowTitle(title: string): void {
		document.title = title;
	}

	async initDeepLinking(): Promise<void> {
		// Deep linking is not supported in the web version
		return await Promise.resolve();
	}
}

class WebDiskStore implements DiskStore {
	constructor() {}

	async set(_key: string, _value: unknown): Promise<void> {
		// TODO: Implement this for the web version
		// This is a no-op for the web version
	}

	async get<T>(key: string, defaultValue: undefined): Promise<T | undefined>;
	async get<T>(key: string, defaultValue: T): Promise<T>;
	async get<T>(key: string, defaultValue?: T): Promise<T | undefined> {
		// TODO: Implement this for the web version
		// This is a no-op for the web version
		const fromCookie = getCookie(`disk-store-override:${key}`);
		try {
			const parsed = fromCookie ? (JSON.parse(fromCookie) as T) : undefined;
			return parsed ?? defaultValue;
		} catch (error) {
			console.error("Error parsing disk store value from cookie", error);
			return defaultValue;
		}
	}
}

export function webLogErrorToFile(error: string) {
	// TODO: Implement this for the web version if needed
	console.error("Logging to file is not supported in web builds.");
	console.error(error);
}
export function webPathSeparator(): string {
	return "/"; // Web uses forward slashes for path
}

async function webReadTextFromClipboard(): Promise<string> {
	return await window.navigator.clipboard.readText();
}

async function webWriteTextToClipboard(text: string): Promise<void> {
	await window.navigator.clipboard.writeText(text);
}

async function webGetAppInfo(): Promise<AppInfo> {
	return await Promise.resolve({
		name: "gitbutler-web",
		version: "0.0.0",
	});
}

async function webHomeDirectory(): Promise<string> {
	return await Promise.resolve("/tmp/gitbutler");
}

async function webJoinPath(pathSegment: string, ...paths: string[]): Promise<string> {
	// TODO: We might want to expose some endpoint in the backend to handle path joining in the right way.
	// This will break on windows
	return await Promise.resolve([pathSegment, ...paths].join(webPathSeparator()));
}

async function webDocumentDir(): Promise<string> {
	// This needs to be implemented for the web version
	return await Promise.resolve("");
}

async function webFilePicker<T extends OpenDialogOptions>(
	options?: T,
): Promise<OpenDialogReturn<T>> {
	const fileInput = document.createElement("input");
	const projectPath = getCookie("PROJECT_PATH") || "";
	fileInput.type = "file";

	if (options?.multiple) {
		fileInput.multiple = true;
	}

	if (options?.directory) {
		fileInput.webkitdirectory = true; // This is a web-specific feature for directory selection
	}

	const promise = new Promise<OpenDialogReturn<T>>((resolve) => {
		fileInput.onchange = () => {
			const files = fileInput.files;
			if (!files || files.length === 0) {
				resolve(null);
				return;
			}

			if (options?.directory) {
				const file = files[0]!;
				const filePath = file.webkitRelativePath || file.name;
				const dirPath = filePath.split("/").slice(0, -1).join("/");
				const absolute = projectPath + "/" + dirPath;
				resolve(absolute as OpenDialogReturn<T>);
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

			const filePath = paths[0];
			resolve(filePath as OpenDialogReturn<T>);
		};
	});

	fileInput.click();
	return await promise;
}

async function webRelaunch(): Promise<void> {
	// The web version does not support relaunching
	throw new Error("Relaunch is not implemented in the web version");
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
		const response = await fetch(`${getApiBaseUrl()}/${command}`, {
			method: "POST",
			headers: {
				"Content-Type": "application/json",
			},
			credentials: "include",
			body: JSON.stringify(params),
		});
		if (response.status === 401) {
			window.location.href = `${getApiBaseUrl()}/auth/login`;
			throw new Error("Authentication required");
		}
		const out: ServerResonse<T> = await response.json();
		if (out.type === "success") {
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
	return "0.0.0";
}

async function webReadFile(_path: string): Promise<Uint8Array> {
	// TODO: Implement this for the web version if needed
	throw new Error("webReadFile is not implemented for the web version");
}

async function webOpenExternalUrl(href: string): Promise<void> {
	window.open(href, "_blank");
	return await Promise.resolve();
}

class WebListener {
	private socket: ReconnectingWebSocket | undefined;
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
			this.socket = new ReconnectingWebSocket(getWsUrl());
			this.socket.addEventListener("message", (event) => {
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

/**
 * Returns the base URL for API calls (without trailing slash).
 *
 * Resolution order:
 * 1. `VITE_BUTLER_API_BASE_URL` — full base URL (e.g. `http://localhost:6978`)
 * 2. `VITE_BUTLER_HOST` + `VITE_BUTLER_PORT` — legacy host/port env vars
 * 3. `` — empty string (same origin, no prefix). When running with `--tunnel`,
 *    the server auto-sets `--base-path=/api` so `VITE_BUTLER_API_BASE_URL`
 *    should be set to `/api` if needed.
 */
export function getApiBaseUrl(): string {
	const base = import.meta.env.VITE_BUTLER_API_BASE_URL;
	if (base) return base.replace(/\/$/, "");

	const host = import.meta.env.VITE_BUTLER_HOST;
	const port = import.meta.env.VITE_BUTLER_PORT;
	if (host && port) {
		const protocol = "http";
		return `${protocol}://${host}:${port}`;
	}

	return "";
}

/**
 * Returns the WebSocket URL for the event stream.
 * Derived from `getApiBaseUrl()` — replaces http(s) with ws(s) and
 * points at `/ws` on the same host (without any `/api` prefix).
 */
function getWsUrl(): string {
	const base = getApiBaseUrl();
	// Empty string or relative URL (e.g. /api) — use window.location.host
	if (!base || base.startsWith("/")) {
		const wsProtocol = window.location.protocol === "https:" ? "wss:" : "ws:";
		return `${wsProtocol}//${window.location.host}${base}/ws`;
	}
	// Absolute URL (e.g. http://localhost:6978 or https://tunnel.com/api) —
	// keep any path component from the base and append /ws.
	const url = new URL(base);
	const wsProtocol = url.protocol === "https:" ? "wss:" : "ws:";
	const basePath = url.pathname.replace(/\/$/, "");
	return `${wsProtocol}//${url.host}${basePath}/ws`;
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
			type: "success";
			subject: T;
	  }
	| {
			type: "error";
			subject: unknown;
	  };
