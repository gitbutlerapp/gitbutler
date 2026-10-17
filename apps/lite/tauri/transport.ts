import { apiParamNames } from "@gitbutler/but-sdk/api-param-names";
import { getVersion } from "@tauri-apps/api/app";
import { join } from "@tauri-apps/api/path";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import type { Handler, HostOnlyKey } from "#electron/endpoint-table.ts";
import type { LiteApiTransport } from "#electron/lite-api.ts";
import type { GUISettings } from "#electron/settings.ts";
import * as ai from "./ai.ts";
import { backend, listenNow, type Subscribe } from "./backend.ts";
import * as nativeMenu from "./native-menu.ts";
import * as notifications from "./notifications.ts";
import * as updater from "./updater.ts";
import * as watcher from "./watcher.ts";

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
	pickDirectory: () => open({ directory: true, multiple: false }),
	readGUISettings: (): GUISettings => {
		const stored = localStorage.getItem(guiSettingsKey);
		return stored === null ? defaultGUISettings : (JSON.parse(stored) as GUISettings);
	},
	writeGUISettings: (settings) => localStorage.setItem(guiSettingsKey, JSON.stringify(settings)),
	updateFeatureFlags: (update) => backend("update_feature_flags", { update }) as Promise<void>,
	...updater.handlers,
	...notifications.handlers,
	watcherStopAll: watcher.handlers.watcherStopAll,
} satisfies { [K in HostOnlyKey]: Handler<K> };

const hostHandlers: Record<string, (...args: Array<never>) => unknown> = {
	...hostOnly,
	...watcher.handlers,
	...nativeMenu.handlers,
	...ai.handlers,
	isFullScreen: () => getCurrentWindow().isFullscreen(),
	pathJoin: (...paths: Array<string>) => join(...paths),
};

const hostChannels: Record<string, Subscribe> = {
	...updater.channels,
	...notifications.channels,
	...ai.channels,
	askpassPrompt: (listener) => listenNow("git_prompt", listener),
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
		const subscribe =
			watcher.watcherChannel(channel) ??
			(Object.hasOwn(hostChannels, channel) ? hostChannels[channel] : undefined);
		return subscribe === undefined ? () => undefined : subscribe(listener);
	},
	platform: platformFromUserAgent(navigator.userAgent),
};
