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
	if (!response.ok) {
		throw Object.assign(new Error(`${url}: ${response.status} ${response.statusText}`), {
			status: response.status,
		});
	}
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

type Listener = (payload: unknown) => void;

/** A project's watcher events arrive on `watcher:<projectId>`, as the server names them. */
const WATCHER_CHANNEL_PREFIX = "watcher:";
const watcherChannel = (projectId: string) => `${WATCHER_CHANNEL_PREFIX}${projectId}`;
const projectIdOf = (channel: string) => channel.slice(WATCHER_CHANNEL_PREFIX.length);

/** The server's own messages; anything else on the socket is a `{ channel, payload }` event. */
type EventsReply = { type: "subscribed" | "rejected"; projectId: string } | { type: "heartbeat" };

/** The server sends a heartbeat every 25s, so this much silence means the socket is dead. */
const EVENTS_IDLE_TIMEOUT = 60_000;

/** One WebSocket to `/events` per page, open while anything listens and reopened when it drops or goes silent. */
const createEventStream = (serverUrl: string) => {
	const listeners = new Map<string, Set<Listener>>();
	let socket: WebSocket | null = null;
	let reconnect: ReturnType<typeof setTimeout> | undefined;
	let idle: ReturnType<typeof setTimeout> | undefined;
	let connecting = false;
	let retryDelay = 1_000;
	// The server has no `/events`, so nothing is retried for the rest of the page.
	let unsupported = false;

	// Before the socket opens, the "open" handler sends whatever is subscribed by then.
	const send = (type: "subscribe" | "unsubscribe", channel: string) => {
		if (socket?.readyState !== WebSocket.OPEN) return;
		socket.send(JSON.stringify({ type, projectId: projectIdOf(channel) }));
	};

	const refresh = (projectId: string) => {
		// Nothing reads `headSha`; the event type alone picks which queries refresh.
		const activity = {
			name: `project://${projectId}/git/activity`,
			payload: { type: "gitActivity", subject: { headSha: "" } },
		};
		for (const listener of listeners.get(watcherChannel(projectId)) ?? []) listener(activity);
	};

	const scheduleReconnect = () => {
		if (listeners.size === 0) return;
		// Jitter keeps pages that lost the same server from reconnecting in lockstep.
		reconnect = setTimeout(
			() => {
				reconnect = undefined;
				void connect();
			},
			retryDelay * (0.5 + Math.random() / 2),
		);
		retryDelay = Math.min(retryDelay * 2, 30_000);
	};

	// A dead connection may never report closing, so it's abandoned rather than awaited.
	const abandon = () => {
		const dead = socket;
		socket = null;
		clearTimeout(idle);
		dead?.close();
		scheduleReconnect();
	};

	// The socket can't carry the bearer header, so it presents a single-use ticket instead.
	const connect = async () => {
		connecting = true;
		let ticket: string;
		try {
			({ ticket } = (await post(`${serverUrl}/events/ticket`, {})) as { ticket: string });
		} catch (error) {
			// A server without the route (a plain local but-server) has no events to wait for.
			const status = (error as { status?: number }).status;
			if (status === 404 || status === 405) unsupported = true;
			else scheduleReconnect();
			return;
		} finally {
			connecting = false;
		}
		if (listeners.size === 0) return;

		const url = new URL(`${serverUrl}/events`, window.location.href);
		url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
		url.searchParams.set("ticket", ticket);

		const current = new WebSocket(url);
		socket = current;
		const stillAlive = () => {
			clearTimeout(idle);
			idle = setTimeout(abandon, EVENTS_IDLE_TIMEOUT);
		};
		current.addEventListener("open", () => {
			retryDelay = 1_000;
			stillAlive();
			// The server forgets a socket's subscriptions when it closes.
			for (const channel of listeners.keys()) send("subscribe", channel);
		});
		current.addEventListener("message", (message) => {
			if (socket !== current) return;
			stillAlive();
			let data: EventsReply | { channel: string; payload: unknown };
			try {
				data = JSON.parse(String(message.data)) as typeof data;
			} catch {
				// Ignore malformed frames, as the server does.
				return;
			}
			if (!("channel" in data)) {
				// Changes from before the subscription went live never arrive as events.
				if (data.type === "subscribed") refresh(data.projectId);
				return;
			}
			for (const listener of listeners.get(data.channel) ?? []) listener(data.payload);
		});
		current.addEventListener("close", () => {
			if (socket !== current) return;
			socket = null;
			clearTimeout(idle);
			scheduleReconnect();
		});
	};

	return (channel: string, listener: Listener): (() => void) => {
		let channelListeners = listeners.get(channel);
		if (!channelListeners) {
			channelListeners = new Set();
			listeners.set(channel, channelListeners);
			send("subscribe", channel);
		}
		channelListeners.add(listener);
		if (socket === null && reconnect === undefined && !connecting && !unsupported) void connect();

		return () => {
			channelListeners.delete(listener);
			if (channelListeners.size > 0) return;
			listeners.delete(channel);
			send("unsubscribe", channel);
			if (listeners.size === 0) socket?.close();
		};
	};
};

export const createHttpTransport = (serverUrl: string): LiteApiTransport => {
	const subscribeToEvents = createEventStream(serverUrl);
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
		watcherSubscribe: (params) => ({
			subscriptionId: crypto.randomUUID(),
			eventChannel: watcherChannel((params as { projectId: string }).projectId),
		}),
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
		// Only watcher events come from the server; other host events have no browser source.
		subscribe: (channel, listener) =>
			channel.startsWith(WATCHER_CHANNEL_PREFIX) ? subscribeToEvents(channel, listener) : () => {},
		platform: browserPlatform(),
		// Only a hosted server serves this page itself; a local one is reached by URL.
		hosted: serverUrl === "",
	};
};
