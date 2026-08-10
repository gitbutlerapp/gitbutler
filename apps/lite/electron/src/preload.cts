import { contextBridge, ipcRenderer } from "electron";
import { exposedEndpoints, localEndpoints } from "./ipc.js";
import type { LiteElectronApi, WatcherSubscribeResult } from "./ipc";
import type { AskpassPromptEvent, WatcherEvent } from "@gitbutler/but-sdk";

// Sandboxed preloads cannot load local modules. Keep these values in sync with tracing.ts.
const ipcTraceCompleteChannel = "lite:ipc-trace-complete";
const ipcTraceWatcherEventChannel = "lite:ipc-trace-watcher-event";
const ipcTraceWatcherHost = "ipc-watcher.localhost";
const ipcTracePathPrefix = "/__ipc/";
const ipcTraceAcceptPrefix = "application/json; trace-id=";
const ipcTraceStreamAcceptPrefix = "text/event-stream; trace-id=";
const ipcTraceWatcherEventsPath = `${ipcTracePathPrefix}watcher-events`;
const maxTraceResponseCharacters = 1_000_000;
const maxTraceArgsCharacters = 4_000;
const maxTraceGateMs = 60_000;
const ipcTraceServerUrl = (() => {
	const devServerUrl = process.env.VITE_DEV_SERVER_URL;
	if (devServerUrl === undefined) return undefined;

	const url = new URL(devServerUrl);
	url.hostname = "ipc.localhost";
	return url;
})();
const ipcTraceWatcherServerUrl = (() => {
	if (ipcTraceServerUrl === undefined) return undefined;

	const url = new URL(ipcTraceServerUrl);
	url.hostname = ipcTraceWatcherHost;
	return url;
})();

interface IpcTrace {
	traceId: string;
	startedAt: number;
	finished: Promise<void>;
	stopWaiting: () => void;
}

const traceSafeArgs = (channel: string, args: Array<unknown>): Array<unknown> => {
	if (channel === "askpassSubmitPromptResponse") {
		const [params, ...rest] = args;
		if (typeof params === "object" && params !== null)
			return [{ ...params, response: "<redacted>" }, ...rest];
	}

	if (channel === "clipboardWriteText") return ["<redacted>"];

	return args;
};

// Lite's IPC arguments and results are JSON-compatible. Errors are the one useful exception: their
// properties are not enumerable, so normalize them before displaying them in DevTools.
const stringifyTracePayload = (
	value: unknown,
	maxCharacters = maxTraceResponseCharacters,
): string => {
	let json: string;

	try {
		json = JSON.stringify(value, (_key, nestedValue: unknown) => {
			if (nestedValue instanceof Error) {
				return {
					name: nestedValue.name,
					message: nestedValue.message,
					stack: nestedValue.stack,
				};
			}
			return nestedValue;
		});
	} catch (error) {
		json = JSON.stringify({ serializationError: String(error) });
	}

	if (json.length <= maxCharacters) return json;
	return JSON.stringify({
		truncated: true,
		originalCharacters: json.length,
		preview: json.slice(0, maxCharacters),
	});
};

const beginIpcTrace = (channel: string, args: Array<unknown>): IpcTrace | undefined => {
	if (ipcTraceServerUrl === undefined) return undefined;

	const traceId = crypto.randomUUID();
	const separatorIndex = channel.indexOf(":");
	const scope = separatorIndex === -1 ? "ipc" : channel.slice(0, separatorIndex);
	const method = separatorIndex === -1 ? channel : channel.slice(separatorIndex + 1);
	const traceUrl = new URL(
		`${ipcTracePathPrefix}${encodeURIComponent(scope)}/${encodeURIComponent(method)}`,
		ipcTraceServerUrl,
	);
	const serializedArgs = stringifyTracePayload(
		traceSafeArgs(channel, args),
		maxTraceArgsCharacters,
	);
	const abortController = new AbortController();
	const startedAt = performance.now();

	const response = fetch(traceUrl, {
		method: "POST",
		headers: { Accept: `${ipcTraceAcceptPrefix}${traceId}` },
		body: serializedArgs,
		signal: abortController.signal,
	});
	const timeout = setTimeout(() => abortController.abort(), maxTraceGateMs);
	const finished = response
		.then((response) => response.arrayBuffer())
		.then(() => undefined)
		.catch(() => undefined)
		.finally(() => clearTimeout(timeout));

	return {
		traceId,
		startedAt,
		finished,
		stopWaiting: () => abortController.abort(),
	};
};

const completeIpcTrace = async (
	trace: IpcTrace | undefined,
	ok: boolean,
	value: unknown,
): Promise<void> => {
	if (trace === undefined) return;

	try {
		const stored = (await ipcRenderer.invoke(ipcTraceCompleteChannel, {
			traceId: trace.traceId,
			ok,
			body: stringifyTracePayload(value),
			durationMs: performance.now() - trace.startedAt,
		})) as unknown;
		if (stored !== true) {
			trace.stopWaiting();
			return;
		}

		await trace.finished;
	} catch {
		trace.stopWaiting();
		// Tracing must never affect the real IPC call.
	}
};

/**
 * The map of subscription IDs to channels and callbacks.
 *
 * This is needed in order to maintain separate changes for each subscription.
 * The subscription ID is known to the UI, but the channel is not.
 */
const watcherListenerBySubscription = new Map<
	string,
	{
		eventChannel: string;
		listener: (_event: Electron.IpcRendererEvent, payload: WatcherEvent) => void;
	}
>();

/** API members implemented below rather than forwarded. */
type SpecialKey =
	| "onAskpassPrompt"
	| "onFullScreenChange"
	| "platform"
	| "watcherSubscribe"
	| "watcherUnsubscribe"
	| "watcherStopAll";

/** The same members under their endpoint names; `platform` has no channel at all. */
const specialNames = [
	"askpassPrompt",
	"fullScreenChange",
	"watcherSubscribe",
	"watcherUnsubscribe",
	"watcherStopAll",
] as const;

type SpecialName = (typeof specialNames)[number];
type ForwardedKey = Exclude<keyof LiteElectronApi, SpecialKey>;
type ListedKey = Exclude<
	(typeof exposedEndpoints)[number] | (typeof localEndpoints)[number],
	SpecialName
>;

/**
 * Wiring a member on only one side fails here: both aliases must resolve to
 * `never`, so a member missing from the lists, or listed without a member,
 * is a compile error rather than a runtime "No handler registered".
 */
type AssertNever<T extends never> = T;
type _EveryMemberIsListed = AssertNever<Exclude<ForwardedKey, ListedKey>>;
type _EveryListedNameHasAMember = AssertNever<Exclude<ListedKey, ForwardedKey>>;

const special = new Set<string>(specialNames);

/**
 * electron declares `invoke` as `Promise<any>`, which spreads unchecked into
 * every caller. Narrowing it to `unknown` — by annotation, not assertion —
 * makes each wire result something a reader has to pin down.
 */
const invoke = async (channel: string, ...args: Array<unknown>): Promise<unknown> => {
	const trace = beginIpcTrace(channel, args);

	try {
		const result = (await ipcRenderer.invoke(channel, ...args)) as unknown;
		await completeIpcTrace(trace, true, result);
		return result;
	} catch (error) {
		await completeIpcTrace(trace, false, error);
		throw error;
	}
};

interface IpcWatcherTraceStream {
	streamId: string;
	connected: boolean;
	active: boolean;
	stop: () => void;
}

let watcherTraceStream: IpcWatcherTraceStream | undefined;

const startWatcherTraceStream = (): void => {
	if (ipcTraceWatcherServerUrl === undefined || watcherTraceStream?.active === true) return;

	const streamId = crypto.randomUUID();
	const abortController = new AbortController();
	const stream: IpcWatcherTraceStream = {
		streamId,
		connected: false,
		active: true,
		stop: () => {
			stream.active = false;
			abortController.abort();
		},
	};
	watcherTraceStream = stream;

	const streamUrl = new URL(ipcTraceWatcherEventsPath, ipcTraceWatcherServerUrl);
	void fetch(streamUrl, {
		headers: { Accept: `${ipcTraceStreamAcceptPrefix}${streamId}` },
		signal: abortController.signal,
	})
		.then(async (response) => {
			if (!response.ok || !stream.active) return;
			stream.connected = true;

			// Drain incrementally: response.text() would retain the stream's entire lifetime in memory.
			const reader = response.body?.getReader();
			if (reader === undefined) return;
			while (!(await reader.read()).done) {
				// DevTools records the server-sent events; preload only needs to consume the bytes.
			}
		})
		.catch(() => undefined)
		.finally(() => {
			stream.connected = false;
			stream.active = false;
		});
};

const stopWatcherTraceStream = (): void => {
	watcherTraceStream?.stop();
	watcherTraceStream = undefined;
};

const traceWatcherEvent = (
	projectId: string,
	subscriptionId: string,
	event: WatcherEvent,
): void => {
	const stream = watcherTraceStream;
	if (stream?.connected !== true) return;

	void ipcRenderer
		.invoke(ipcTraceWatcherEventChannel, {
			streamId: stream.streamId,
			type: event.payload.type,
			body: stringifyTracePayload({
				projectId,
				subscriptionId,
				payload: event.payload,
			}),
		})
		.catch(() => undefined);
};

/**
 * Every forwarded member hands its arguments to the channel of the same
 * name, so they are generated from the lists rather than written out. The
 * cast is sound because arguments and results are typed at every call site
 * through `LiteElectronApi`.
 */
const forwarders = Object.fromEntries(
	[...exposedEndpoints, ...localEndpoints]
		.filter((name) => !special.has(name))
		.map((name) => [name, (...args: Array<unknown>) => invoke(name, ...args)]),
) as Omit<LiteElectronApi, SpecialKey>;

const api: LiteElectronApi = {
	...forwarders,
	onAskpassPrompt: (callback) => {
		const listener = (_event: Electron.IpcRendererEvent, payload: AskpassPromptEvent) => {
			callback(payload);
		};
		ipcRenderer.on("askpassPrompt", listener);
		return () => ipcRenderer.removeListener("askpassPrompt", listener);
	},
	onFullScreenChange: (callback) => {
		const listener = (_event: Electron.IpcRendererEvent, fullScreen: boolean) => {
			callback(fullScreen);
		};
		ipcRenderer.on("fullScreenChange", listener);
		return () => ipcRenderer.removeListener("fullScreenChange", listener);
	},
	/**
	 * Subscribe to a project.
	 *
	 * This sets up a listener to project updates from the Rust-end.
	 *
	 * **Usage**
	 * It's expected that one window has max one subscription per project, although it is possible to have multiple.
	 * The node-end of the application will deduplicate project watchers (there will only ever be one watcher) but
	 * there is no deduplication in terms of project subscriptions.
	 *
	 * The responsability of subscribing once and correctly unsubscribing to a project is shifted to the UI.
	 *
	 * @param projectId - The ID of the project to subscribe to.
	 * @param callback - The callback function to pass the event information to.
	 * @returns A subscription ID.
	 */
	watcherSubscribe: async (projectId, callback) => {
		const { subscriptionId, eventChannel } = (await invoke("watcherSubscribe", {
			projectId,
		})) as WatcherSubscribeResult;
		startWatcherTraceStream();
		const listener = (_event: Electron.IpcRendererEvent, payload: WatcherEvent) => {
			traceWatcherEvent(projectId, subscriptionId, payload);
			callback(payload);
		};
		watcherListenerBySubscription.set(subscriptionId, { eventChannel, listener });
		ipcRenderer.on(eventChannel, listener);
		return subscriptionId;
	},
	/**
	 * Stop watching a project.
	 *
	 * Remove the listener for a given subscription channel.
	 * If this is the last subscription to a project, the watcher will stop.
	 * @param subscriptionId
	 */
	watcherUnsubscribe: async (subscriptionId) => {
		const registration = watcherListenerBySubscription.get(subscriptionId);
		if (registration) {
			ipcRenderer.removeListener(registration.eventChannel, registration.listener);
			watcherListenerBySubscription.delete(subscriptionId);
			if (watcherListenerBySubscription.size === 0) stopWatcherTraceStream();
		}
		return invoke("watcherUnsubscribe", { subscriptionId }) as Promise<boolean>;
	},
	/**
	 * Stop all watchers.
	 *
	 * Remove all subscription listners and stop all watchers.
	 */
	watcherStopAll: async () => {
		for (const { eventChannel, listener } of watcherListenerBySubscription.values())
			ipcRenderer.removeListener(eventChannel, listener);

		watcherListenerBySubscription.clear();
		stopWatcherTraceStream();
		return invoke("watcherStopAll") as Promise<number>;
	},
	platform: process.platform,
};

contextBridge.exposeInMainWorld("lite", api);
