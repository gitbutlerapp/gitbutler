import type { WatcherPayload } from "@gitbutler/but-sdk";
import { apiParamNames } from "@gitbutler/but-sdk/api-param-names";
import { getVersion } from "@tauri-apps/api/app";
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { join } from "@tauri-apps/api/path";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import type { Handler, HostOnlyKey } from "#electron/endpoint-table.ts";
import type { WatcherSubscribeParams, WatcherSubscribeResult } from "#electron/ipc.ts";
import type { LiteApiTransport } from "#electron/lite-api.ts";
import type { GUISettings } from "#electron/settings.ts";

const toError = (error: unknown): Error => {
	if (error instanceof Error) return error;
	if (typeof error === "object" && error !== null && "message" in error)
		return new Error(String(error.message));
	return new Error(String(error));
};

const backend = async (command: string, args?: Record<string, unknown>): Promise<unknown> => {
	try {
		return await tauriInvoke(command, args);
	} catch (error) {
		throw toError(error);
	}
};

const commandName = (endpoint: string): string =>
	endpoint.replaceAll(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);

const endpointArgs = (
	names: ReadonlyArray<string>,
	args: Array<unknown>,
): Record<string, unknown> => {
	const [onlyName] = names;
	if (names.length === 1 && onlyName !== undefined) return { [onlyName]: args[0] };
	return (args[0] ?? {}) as Record<string, unknown>;
};

const listenNow = (event: string, listener: (payload: unknown) => void): (() => void) => {
	const unlisten = listen(event, ({ payload }) => listener(payload));
	return () => void unlisten.then((stop) => stop());
};

// Tauri emits the Svelte app's raw payloads; Lite reads the SDK's tagged ones.
// No changed paths are sent, which Lite treats as "unknown, refresh broadly".
const watcherPayloadByEvent: Record<string, (payload: never) => WatcherPayload> = {
	"git/fetch": () => ({ type: "gitFetch", subject: null }),
	"git/head": (subject) => ({ type: "gitHead", subject }),
	"git/activity": (subject) => ({ type: "gitActivity", subject }),
	"workspace-activity": () => ({ type: "workspaceActivity", subject: null }),
	"external-invalidation": (subject) => ({ type: "externalInvalidation", subject }),
	worktree_changes: (changes) => ({
		type: "worktreeChanges",
		subject: { changes, changedPaths: [] },
	}),
	linked_worktree_changes: (subject: { worktree: string; changes: never }) => ({
		type: "linkedWorktreeChanges",
		subject: { ...subject, changedPaths: [] },
	}),
};

const watcherChannelPrefix = "watcher:";

const subscribeToWatcher = (projectId: string, listener: (payload: unknown) => void) => {
	const unsubscribers = Object.entries(watcherPayloadByEvent).map(([suffix, toPayload]) => {
		const name = `project://${projectId}/${suffix}`;
		return listenNow(name, (payload) => listener({ name, payload: toPayload(payload as never) }));
	});
	return () => {
		for (const unsubscribe of unsubscribers) unsubscribe();
	};
};

const tauriEventByChannel: Record<string, string> = { askpassPrompt: "git_prompt" };

const guiSettingsKey = "gitbutler-lite:gui-settings";
const defaultGUISettings: GUISettings = { version: 1 };

const isWebUrl = (url: string): boolean => ["https:", "http:"].includes(new URL(url).protocol);

const hostOnly = {
	askpassSubmitPromptResponse: ({ id, response }) =>
		backend("submit_prompt_response", { id, response }) as Promise<void>,
	clipboardWriteText: (text) => navigator.clipboard.writeText(text),
	getAppSettings: () => backend("get_app_settings") as ReturnType<Handler<"getAppSettings">>,
	getVersion: () => getVersion(),
	isPackaged: () => !import.meta.env.DEV,
	installCli: () => backend("install_cli").then(() => true),
	openInWebBrowser: async (url) => {
		if (!isWebUrl(url)) throw new Error(`URL ${url} has an unsupported protocol`);
		await backend("open_url", { url });
	},
	showItemInFolder: () => {
		throw new Error("Showing items in the file manager is not available in Tauri yet");
	},
	showNotification: () => undefined,
	pickDirectory: () => open({ directory: true, multiple: false }),
	watcherStopAll: () => 0,
	readGUISettings: (): GUISettings => {
		const stored = localStorage.getItem(guiSettingsKey);
		return stored === null ? defaultGUISettings : (JSON.parse(stored) as GUISettings);
	},
	writeGUISettings: (settings) => localStorage.setItem(guiSettingsKey, JSON.stringify(settings)),
	updateFeatureFlags: (update) => backend("update_feature_flags", { update }) as Promise<void>,
	getUpdateStatus: () => ({ _tag: "Idle" }),
	checkForUpdates: () => ({ _tag: "Unavailable" }),
	downloadUpdate: () => undefined,
	installUpdate: () => undefined,
} satisfies { [K in HostOnlyKey]: Handler<K> };

const imperative = {
	isFullScreen: () => getCurrentWindow().isFullscreen(),
	pathJoin: (...paths: Array<string>) => join(...paths),
	showNativeMenu: () => null,
	streamAiResponse: () => {
		throw new Error("Streaming AI responses is not available in Tauri yet");
	},
	watcherSubscribe: async ({
		projectId,
	}: WatcherSubscribeParams): Promise<WatcherSubscribeResult> => {
		await backend("set_project_active", { id: projectId });
		return {
			subscriptionId: crypto.randomUUID(),
			eventChannel: `${watcherChannelPrefix}${projectId}`,
		};
	},
	// The window's watcher follows its active project, not its subscriptions.
	watcherUnsubscribe: () => true,
};

const hostHandlers: Record<string, (...args: Array<never>) => unknown> = {
	...hostOnly,
	...imperative,
};

const sdkParamNames: Record<string, ReadonlyArray<string>> = apiParamNames;

const platformFromUserAgent = (userAgent: string): string => {
	if (userAgent.includes("Mac")) return "darwin";
	if (userAgent.includes("Windows")) return "win32";
	return "linux";
};

export const tauriTransport: LiteApiTransport = {
	invoke: async (channel, ...args) => {
		if (Object.hasOwn(hostHandlers, channel))
			return hostHandlers[channel]?.(...(args as Array<never>));
		const names = Object.hasOwn(sdkParamNames, channel) ? sdkParamNames[channel] : undefined;
		if (names === undefined) throw new Error(`No Tauri handler for ${channel}`);
		return backend(commandName(channel), endpointArgs(names, args));
	},
	subscribe: (channel, listener) => {
		if (channel.startsWith(watcherChannelPrefix))
			return subscribeToWatcher(channel.slice(watcherChannelPrefix.length), listener);
		const event = tauriEventByChannel[channel];
		return event === undefined ? () => undefined : listenNow(event, listener);
	},
	platform: platformFromUserAgent(navigator.userAgent),
};
