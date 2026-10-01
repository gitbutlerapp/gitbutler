/**
 * Lite's transport over but-server's HTTP API. SDK endpoints go to
 * `POST /sdk/{endpoint}` with their named parameters; the few members a
 * desktop host answers itself get browser stand-ins here.
 */
import { apiParamNames } from "@gitbutler/but-sdk/api-param-names";
import type { LiteApiTransport } from "#electron/lite-api.ts";
import type { GUISettings } from "#electron/settings.ts";

type ServerResponse = { type: "success"; subject: unknown } | { type: "error"; subject: unknown };

const GUI_SETTINGS_KEY = "lite.guiSettings";
const TOKEN_KEY = "lite.serverToken";

/** A hosted server's token arrives once as `?token=` and is kept for later visits. */
const serverToken = (): string | null => {
	const url = new URL(window.location.href);
	const fromUrl = url.searchParams.get("token");
	try {
		if (fromUrl !== null) {
			localStorage.setItem(TOKEN_KEY, fromUrl);
			url.searchParams.delete("token");
			window.history.replaceState(null, "", url);
		}
		return localStorage.getItem(TOKEN_KEY);
	} catch {
		return fromUrl;
	}
};

const token = serverToken();

const errorFrom = (subject: unknown): Error => {
	const message =
		typeof subject === "object" && subject !== null && "message" in subject
			? String(subject.message)
			: JSON.stringify(subject);
	return Object.assign(new Error(message), { subject });
};

const post = async (url: string, body: unknown): Promise<unknown> => {
	const response = await fetch(url, {
		method: "POST",
		headers: {
			"content-type": "application/json",
			...(token === null ? {} : { authorization: `Bearer ${token}` }),
		},
		body: JSON.stringify(body),
	});
	if (!response.ok) throw new Error(`${url}: ${response.status} ${response.statusText}`);
	const result = (await response.json()) as ServerResponse;
	if (result.type === "error") throw errorFrom(result.subject);
	return result.subject;
};

/** Lite sends a lone argument as itself; the server always wants it named. */
const namedParams = (endpoint: keyof typeof apiParamNames, args: Array<unknown>): unknown => {
	const names: ReadonlyArray<string> = apiParamNames[endpoint];
	if (names.length === 0) return {};
	if (names.length === 1) return { [names[0] as string]: args[0] };
	return args[0];
};

const browserPlatform = (): string => {
	const platform = navigator.platform.toLowerCase();
	if (platform.startsWith("mac")) return "darwin";
	if (platform.startsWith("win")) return "win32";
	return "linux";
};

const unavailable = (channel: string) => () =>
	Promise.reject(new Error(`${channel} is not available in the browser`));

export const createHttpTransport = (serverUrl: string): LiteApiTransport => {
	const local: Record<string, (...args: Array<unknown>) => unknown> = {
		clipboardWriteText: (text) => navigator.clipboard.writeText(String(text)),
		openInWebBrowser: (url) => {
			window.open(String(url), "_blank", "noopener");
		},
		getAppSettings: () => post(`${serverUrl}/get_app_settings`, {}),
		updateFeatureFlags: (update) => post(`${serverUrl}/update_feature_flags`, { update }),
		getVersion: () => "web",
		isPackaged: () => false,
		isFullScreen: () => false,
		pathJoin: (...paths) => paths.map(String).join("/").replace(/\/+/g, "/"),
		// No popup host yet: menus stay closed.
		showNativeMenu: () => null,
		pickDirectory: async () => {
			const { path } = (await post(`${serverUrl}/pick_directory`, {})) as { path: string | null };
			return path;
		},
		readGUISettings: (): GUISettings => {
			try {
				const stored = localStorage.getItem(GUI_SETTINGS_KEY);
				if (stored !== null) return JSON.parse(stored) as GUISettings;
			} catch {
				// Unreadable storage falls back to defaults.
			}
			return { version: 1 };
		},
		writeGUISettings: (settings) => {
			try {
				localStorage.setItem(GUI_SETTINGS_KEY, JSON.stringify(settings));
			} catch {
				// Settings just don't persist.
			}
		},
		// Live updates need a server event stream; until then views refresh on their own reads.
		watcherSubscribe: () => ({ subscriptionId: crypto.randomUUID(), eventChannel: "watcher" }),
		watcherUnsubscribe: () => true,
		watcherStopAll: () => 0,
		getUpdateStatus: () => ({ _tag: "Idle" }),
		checkForUpdates: () => ({ _tag: "Unavailable" }),
		downloadUpdate: () => undefined,
		installUpdate: () => undefined,
		askpassSubmitPromptResponse: unavailable("askpassSubmitPromptResponse"),
		installCli: unavailable("installCli"),
		showItemInFolder: unavailable("showItemInFolder"),
		showNotification: () => undefined,
		streamAiResponse: unavailable("streamAiResponse"),
	};

	return {
		invoke: async (channel, ...args) => {
			const handler = local[channel];
			if (handler) return handler(...args);
			if (channel in apiParamNames) {
				const endpoint = channel as keyof typeof apiParamNames;
				return post(`${serverUrl}/sdk/${channel}`, namedParams(endpoint, args));
			}
			throw new Error(`Unknown endpoint: ${channel}`);
		},
		// No host events reach the browser yet.
		subscribe: () => () => {},
		platform: browserPlatform(),
	};
};
