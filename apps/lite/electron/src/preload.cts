import { contextBridge, ipcRenderer } from "electron";
import { exposedEndpoints, localEndpoints } from "./ipc.js";
import type { LiteElectronApi, WatcherSubscribeResult } from "./ipc";
import type { AskpassPromptEvent, WatcherEvent } from "@gitbutler/but-sdk";
import type { StreamAiResponseToken } from "./ipc";

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
	| "streamAiResponse"
	| "watcherSubscribe"
	| "watcherUnsubscribe"
	| "watcherStopAll";

/** The same members under their endpoint names; `platform` has no channel at all. */
const specialNames = [
	"askpassPrompt",
	"fullScreenChange",
	"streamAiResponse",
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
const invoke = (channel: string, ...args: Array<unknown>): Promise<unknown> =>
	ipcRenderer.invoke(channel, ...args);

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
	streamAiResponse: async (systemMessage, prompt, onToken) => {
		const requestId = crypto.randomUUID();
		const listener = (_event: Electron.IpcRendererEvent, payload: StreamAiResponseToken) => {
			if (payload.requestId === requestId) onToken(payload.token);
		};
		ipcRenderer.on("streamAiResponseToken", listener);
		try {
			return (await invoke("streamAiResponse", { requestId, systemMessage, prompt })) as string;
		} finally {
			ipcRenderer.removeListener("streamAiResponseToken", listener);
		}
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
		const listener = (_event: Electron.IpcRendererEvent, payload: WatcherEvent) => {
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
		return invoke("watcherStopAll") as Promise<number>;
	},
	platform: process.platform,
};

contextBridge.exposeInMainWorld("lite", api);
