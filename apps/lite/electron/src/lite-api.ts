import type { AskpassPromptEvent, WatcherEvent } from "@gitbutler/but-sdk";
import { exposedEndpoints, localEndpoints } from "./ipc.js";
import type { LiteElectronApi, StreamAiResponseToken, WatcherSubscribeResult } from "./ipc.js";

/**
 * What a host must provide to build the renderer's api: one request/response
 * function, one event channel, and the platform it runs on. Electron backs
 * these with the preload's ipcRenderer; other hosts with their own channel.
 */
export type LiteApiTransport = {
	/** Send one request and await its result. */
	invoke: (channel: string, ...args: Array<unknown>) => Promise<unknown>;
	/** Hear every event on a named channel until the returned function runs. */
	subscribe: (channel: string, listener: (payload: unknown) => void) => () => void;
	platform: LiteElectronApi["platform"];
};

/** API members implemented below rather than forwarded. */
type SpecialKey =
	| "onAskpassPrompt"
	| "onDeepLink"
	| "onFullScreenChange"
	| "platform"
	| "streamAiResponse"
	| "watcherSubscribe"
	| "watcherUnsubscribe"
	| "watcherStopAll";

/** The same members under their endpoint names; `platform` has no channel at all. */
const specialNames = [
	"askpassPrompt",
	"deepLink",
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
 * A member missing from the endpoint lists, or listed without a member, is
 * a compile error here rather than a runtime "No handler registered".
 */
type AssertNever<T extends never> = T;
type _EveryMemberIsListed = AssertNever<Exclude<ForwardedKey, ListedKey>>;
type _EveryListedNameHasAMember = AssertNever<Exclude<ListedKey, ForwardedKey>>;

const special = new Set<string>(specialNames);

/**
 * Builds the renderer's whole api over a transport. Forwarded members are
 * generated from the endpoint lists — each sends its arguments to the
 * channel of the same name — so a partial api cannot be built. The cast is
 * sound because every call site is typed through `LiteElectronApi`.
 */
export const createLiteApi = ({
	invoke,
	subscribe,
	platform,
}: LiteApiTransport): LiteElectronApi => {
	/** Unsubscribers per watcher subscription; the UI knows ids, not channels. */
	const watcherUnsubscribeBySubscription = new Map<string, () => void>();

	const forwarders = Object.fromEntries(
		[...exposedEndpoints, ...localEndpoints]
			.filter((name) => !special.has(name))
			.map((name) => [name, (...args: Array<unknown>) => invoke(name, ...args)]),
	) as Omit<LiteElectronApi, SpecialKey>;

	return {
		...forwarders,
		onAskpassPrompt: (callback) =>
			subscribe("askpassPrompt", (payload) => {
				callback(payload as AskpassPromptEvent);
			}),
		onDeepLink: (callback) =>
			subscribe("deepLink", (payload) => {
				callback(payload as string);
			}),
		onFullScreenChange: (callback) =>
			subscribe("fullScreenChange", (payload) => {
				callback(payload as boolean);
			}),
		streamAiResponse: async (systemMessage, prompt, onToken) => {
			const requestId = crypto.randomUUID();
			const unsubscribe = subscribe("streamAiResponseToken", (payload) => {
				const token = payload as StreamAiResponseToken;
				if (token.requestId === requestId) onToken(token.token);
			});
			try {
				return (await invoke("streamAiResponse", { requestId, systemMessage, prompt })) as string;
			} finally {
				unsubscribe();
			}
		},
		/**
		 * Listen to backend events for a project. The host keeps one watcher
		 * per project no matter how many subscriptions exist, but does not
		 * deduplicate subscriptions — subscribing once and unsubscribing is
		 * the UI's job. Returns an id for `watcherUnsubscribe`.
		 */
		watcherSubscribe: async (projectId, callback) => {
			const { subscriptionId, eventChannel } = (await invoke("watcherSubscribe", {
				projectId,
			})) as WatcherSubscribeResult;
			const unsubscribe = subscribe(eventChannel, (payload) => {
				callback(payload as WatcherEvent);
			});
			watcherUnsubscribeBySubscription.set(subscriptionId, unsubscribe);
			return subscriptionId;
		},
		/** Stop listening; the project's watcher stops with its last subscription. */
		watcherUnsubscribe: async (subscriptionId) => {
			watcherUnsubscribeBySubscription.get(subscriptionId)?.();
			watcherUnsubscribeBySubscription.delete(subscriptionId);
			return invoke("watcherUnsubscribe", { subscriptionId }) as Promise<boolean>;
		},
		/** Drop every subscription and stop every watcher. */
		watcherStopAll: async () => {
			for (const unsubscribe of watcherUnsubscribeBySubscription.values()) unsubscribe();

			watcherUnsubscribeBySubscription.clear();
			return invoke("watcherStopAll") as Promise<number>;
		},
		platform,
	};
};
