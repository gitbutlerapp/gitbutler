import { checkForUpdates, registerUpdater, setAutoUpdateEnabled } from "./updater.js";
import WatcherManager from "./watcher.js";
import * as sdk from "@gitbutler/but-sdk";
import { apiParamNames } from "@gitbutler/but-sdk/api-param-names";
import {
	exposedEndpoints,
	type PayloadFor,
	type Endpoint,
	type LiteElectronApi,
	type ShowNativeMenuParams,
	type StreamAiResponseParams,
	type WatcherSubscribeParams,
	type WatcherUnsubscribeParams,
	type NativeMenuPopupItem,
} from "./ipc.js";
import {
	askpassInit,
	askpassSubmitPromptResponse,
	initApplicationNamespace,
} from "@gitbutler/but-sdk";
import {
	app,
	BrowserWindow,
	clipboard,
	ipcMain,
	Menu,
	nativeTheme,
	net,
	protocol,
	session,
	shell,
	type MenuItemConstructorOptions,
} from "electron";
import {
	REACT_DEVELOPER_TOOLS,
	REDUX_DEVTOOLS,
	installExtension,
} from "electron-devtools-installer";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { type GUISettings, readSettings, writeSettings } from "./settings.js";

const isHeadless = process.env.GITBUTLER_LITE_HEADLESS === "true";
if (isHeadless && process.platform === "darwin") app.setActivationPolicy("accessory");

// Do this early before any APIs that depend upon it are called. Likewise take care in imported
// modules.
if (!app.isPackaged) app.setName("GitButler Lite Dev");

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);

// [ref:lite_default_settings]
const applyGUISettings = (settings: GUISettings): void => {
	nativeTheme.themeSource = settings.theme ?? "system";
	setAutoUpdateEnabled(settings.autoUpdate ?? true);
};

// Permissions in this array are allowed by default for trusted origins, without prompting the user for input.
const trustedOriginDefaultPermissions: Array<
	| "clipboard-read"
	| "clipboard-sanitized-write"
	| "display-capture"
	| "fullscreen"
	| "geolocation"
	| "idle-detection"
	| "media"
	| "mediaKeySystem"
	| "midi"
	| "midiSysex"
	| "notifications"
	| "pointerLock"
	| "keyboardLock"
	| "openExternal"
	| "speaker-selection"
	| "storage-access"
	| "top-level-storage-access"
	| "window-management"
	| "unknown"
	| "fileSystem"
> = ["clipboard-sanitized-write"] as const;

/**
 * Hosts allowed to supply images, shared by both policies.
 *
 * Both GitHub attachments and GitButler avatars are served by a host that 302s to a
 * bucket, and CSP checks every hop — hence the bucket names alongside the app ones.
 */
const imgSrc = [
	"'self'",
	"data:",
	"https://*.gravatar.com",
	"https://*.githubusercontent.com",
	"https://github.com",
	"https://github-production-user-asset-6210df.s3.amazonaws.com",
	"https://app.gitbutler.com",
	"https://app.staging.gitbutler.com",
	"https://gitbutler-public.s3.amazonaws.com",
].join(" ");

const liteProtocolScheme = "but";
const liteProtocolHost = "app";
const contentRootURL = pathToFileURL(path.join(currentDirPath, "../ui"));
const askpassExecutableName =
	process.platform === "win32" ? "gitbutler-git-askpass.exe" : "gitbutler-git-askpass";

// Custom scheme to serve files. This is necessary for two reasons:
//
// 1. Security, as serving via file:// opens up a wider attack surface than is desirable (see https://www.electronjs.org/docs/latest/tutorial/security#18-avoid-usage-of-the-file-protocol-and-prefer-usage-of-custom-protocols)
// 2. The ability to reload the page when we've set a route that does not correspond to a file we can actually serve
protocol.registerSchemesAsPrivileged([
	{
		scheme: liteProtocolScheme,
		privileges: {
			standard: true,
			secure: true,
			supportFetchAPI: true,
		},
	},
]);

const registerLiteProtocolHandler = () => {
	// Handler based on the examples in https://www.electronjs.org/docs/latest/api/protocol#protocolhandlescheme-handler
	protocol.handle(liteProtocolScheme, async (req) => {
		const { host, pathname } = new URL(req.url);

		// Our bundle is served with a primary index.html and a flat assets directory, so there's
		// no need for relative directory traversal to serve our content at this time. We can
		// therefore trivially prevent path traversal by simply disallowing any ..
		//
		// Don't name files with any intermediate .. and we don't need to make this check account for that :)
		//
		// In addition, we only have the single host to serve from for now.
		if (pathname.includes("..") || host !== liteProtocolHost) {
			return new Response("Not found", {
				status: 404,
				headers: { "content-type": "text/html" },
			});
		}

		// We default to serving the index file unless the pathname indicates it's an asset. This is
		// important to be compatible with React Router's "soft navigation" where it changes the
		// location to track where you are in the app, but it's still an SPA with only an index file
		// to actually serve from the backend. For example, if the user navigates somewhere and then
		// reloads the page, we should still serve up the index file, and React Router will handle the
		// rest by reading the pathname.
		const urlToServe = new URL(contentRootURL);
		urlToServe.pathname += pathname.startsWith("/assets/") ? pathname : "/index.html";

		return net.fetch(urlToServe.toString());
	});
};

const askpassBinDir = (): string =>
	app.isPackaged
		? path.join(process.resourcesPath, "bin")
		: path.join(currentDirPath, "../../resources/bin");

const configureAskpass = (): void => {
	if (app.isPackaged)
		process.env.GITBUTLER_ASKPASS_BIN = path.join(askpassBinDir(), askpassExecutableName);
	else process.env.GITBUTLER_ASKPASS_BIN ??= path.join(askpassBinDir(), askpassExecutableName);

	try {
		askpassInit((err, event) => {
			if (err) {
				// oxlint-disable-next-line no-console
				console.error(`Error encountered while initializing askpass:\n${err}`);
				return;
			}

			// Send the prompt to all windows.
			// TODO: Probably not what we want if we have multiple windows. We should
			// figure out how to send it to the right one.
			for (const window of BrowserWindow.getAllWindows())
				window.webContents.send("askpassPrompt", event);
		});
	} catch (err) {
		// oxlint-disable-next-line no-console
		console.error(`Error encountered while configuring askpass:\n${String(err)}`);
	}
};

// Dev-only runtime icons path. Packaged builds rely on electron-builder, which uses the release
// icons under `resources/icons`, so dev gets a visually distinct set of its own.
const iconsPath = path.join(currentDirPath, "../../resources/icons-dev");

function getWindowIcon(): string | undefined {
	if (app.isPackaged) return undefined;

	let iconPath: string;

	switch (os.platform()) {
		case "win32":
			iconPath = path.join(iconsPath, "windows/icon.ico");
			break;
		case "darwin":
			return undefined;
		default:
			iconPath = path.join(iconsPath, "linux/icons/256x256.png");
			break;
	}

	return fs.existsSync(iconPath) ? iconPath : undefined;
}

function getMacDockIcon(): string | undefined {
	const candidates = [
		path.join(iconsPath, "macos/1024x1024.png"),
		path.join(iconsPath, "macos/512x512.png"),
		path.join(iconsPath, "macos/256x256.png"),
	];

	return candidates.find((c) => fs.existsSync(c));
}

const buildNativeMenuTemplate = (
	items: Array<NativeMenuPopupItem>,
	onItem: (itemId: string) => void,
): Array<MenuItemConstructorOptions> =>
	items.map((item): MenuItemConstructorOptions => {
		if (item._tag === "Separator") return { type: "separator" };
		const itemId = item.itemId;

		return {
			label: item.label,
			accelerator: item.accelerator,
			type: item.checked !== undefined ? "checkbox" : undefined,
			checked: item.checked,
			enabled: item.enabled,
			click: itemId !== undefined ? () => onItem(itemId) : undefined,
			submenu: item.submenu ? buildNativeMenuTemplate(item.submenu, onItem) : undefined,
		};
	});

// Returns true if the `url` is from an origin we trust to perform privileged actions such as executing IPC commands.
const isTrustedLocalOrigin = (url: URL | null) =>
	url !== null &&
	(app.isPackaged
		? url.protocol === `${liteProtocolScheme}:` && url.host === liteProtocolHost
		: url.protocol === "http:" && url.host === "127.0.0.1:5173");

const newUrlOrNull = (url: string): URL | null => {
	try {
		return new URL(url);
	} catch {
		return null;
	}
};

/** Members the renderer implements itself; they have no main-side handler. */
type RendererOnlyKey = "onAskpassPrompt" | "onFullScreenChange" | "platform";

/** Handlers needing the IPC event itself, or taking variadic arguments. */
type ImperativeKey =
	| "isFullScreen"
	| "pathJoin"
	| "showNativeMenu"
	| "streamAiResponse"
	| "watcherSubscribe"
	// The preload wraps the id in an object, so the payload is not the argument.
	| "watcherUnsubscribe";

type TableKey = Exclude<keyof LiteElectronApi, RendererOnlyKey | ImperativeKey>;

/**
 * A handler takes what the renderer sends and returns what it expects, both
 * read off `LiteElectronApi`, so a payload or result that drifts from the
 * renderer's view is a compile error.
 */
type Handler<K extends TableKey> = LiteElectronApi[K] extends (params: infer P) => infer R
	? (params: P) => R | Awaited<R>
	: never;

/**
 * Members the main process answers itself: electron's own capabilities, and
 * the one endpoint that is not `#[but_api]`. Listing one here is what takes
 * it out of the derived set.
 */
const ipcHandlerOverrides = {
	askpassSubmitPromptResponse: ({ id, response }) => askpassSubmitPromptResponse(id, response),
	clipboardWriteText: (text) => {
		clipboard.writeText(text, "clipboard");
	},
	getVersion: () => app.getVersion(),
	getAiConfiguration: () => sdk.getAiConfiguration(),
	openInWebBrowser: (url) => {
		// shell.openExternal() is powerful and dangerous. For example, on macOS you can launch a
		// program with shell.openExternal("file:///Applications/Numbers.app"). Similarly bad
		// things are possible on Windows and Linux.
		//
		// We need to be able to open relatively arbitrary URLs so we can't lock this down too much,
		// but we can at least make sure the URL is a reasonable protocol so we don't allow e.g.
		// "file:///Applications/Numbers.app" to pass through.
		//
		// https://www.electronjs.org/docs/latest/tutorial/security#15-do-not-use-shellopenexternal-with-untrusted-content
		const protocol = newUrlOrNull(url)?.protocol ?? "";
		if (!["https:", "http:"].includes(protocol))
			throw new Error(`URL ${url} with unsupported protocol ${protocol}`);

		return shell.openExternal(url);
	},
	watcherStopAll: () => WatcherManager.getInstance().stopAllWatchersForShutdown(),
	readGUISettings: () => readSettings(),
	resetAiConfiguration: () => sdk.resetAiConfiguration(),
	updateAiConfiguration: (update) => sdk.updateAiConfiguration(update),
	writeGUISettings: async (settings) => {
		applyGUISettings(settings);
		await writeSettings(settings);
	},
} satisfies { [K in TableKey]?: Handler<K> };

type OverrideKey = keyof typeof ipcHandlerOverrides;
type DerivedKey = Exclude<TableKey, OverrideKey>;

/** Narrowing rather than asserting: an exposed endpoint may be either. */
const isOverride = (key: Endpoint): key is Endpoint & OverrideKey => key in ipcHandlerOverrides;

/**
 * Every other endpoint reads its arguments out of the payload by name, so
 * it cannot pass them in the wrong order — which a hand-written call can,
 * silently: `commitMoveChangesBetween` takes source and destination commit
 * ids that are both strings.
 */
const derivedHandler =
	(key: DerivedKey) =>
	(params: unknown): unknown => {
		const names: ReadonlyArray<string> = apiParamNames[key];
		const call = sdk[key] as (...args: Array<unknown>) => unknown;
		// A lone argument is sent as itself; the rest arrive as a payload.
		return names.length === 1
			? call(params)
			: call(...names.map((name) => (params as Record<string, unknown>)[name]));
	};

type PayloadOf<K extends TableKey> = Parameters<LiteElectronApi[K]>[0];

/**
 * A derived handler can only supply arguments its payload carries, so the
 * multi-argument ones must carry every name and the single-argument ones
 * must be the argument. Anything else has to be an override.
 */
type CannotSupplyItsArguments = {
	[K in DerivedKey]: (typeof apiParamNames)[K]["length"] extends 0
		? never
		: (typeof apiParamNames)[K]["length"] extends 1
			? PayloadOf<K> extends Parameters<(typeof sdk)[K]>[0]
				? never
				: K
			: PayloadOf<K> extends PayloadFor<K>
				? never
				: K;
}[DerivedKey];
type AssertNever<T extends never> = T;
type _EveryDerivedHandlerCanSupplyItsArguments = AssertNever<CannotSupplyItsArguments>;

const registerIpcHandlers = (): void => {
	const senderValidatingHandle: typeof ipcMain.handle = (channel, listener) => {
		const senderValidatingListener: typeof listener = (event, ...args) => {
			// Validate that the frame is from a trusted origin. This is crucial to prevent unauthorized
			// access to the IPC bridge if we ever render non-local content.
			//
			// See https://www.electronjs.org/docs/latest/tutorial/security#17-validate-the-sender-of-all-ipc-messages
			const isSenderFrameTrusted =
				event.senderFrame !== null && isTrustedLocalOrigin(newUrlOrNull(event.senderFrame.url));
			if (isSenderFrameTrusted)
				// eslint-disable-next-line @typescript-eslint/no-unsafe-argument @typescript-eslint/no-unsafe-return
				return listener(event, ...args);

			// oxlint-disable-next-line no-console
			console.error(`Rejecting untrusted sender frame ${event.senderFrame?.url ?? "<unknown>"}`);
			return null;
		};

		ipcMain.handle(channel, senderValidatingListener);
	};

	for (const key of exposedEndpoints) {
		if (isOverride(key)) continue;
		senderValidatingHandle(key, (_e, params: unknown) => derivedHandler(key)(params));
	}
	for (const [name, handler] of Object.entries(ipcHandlerOverrides)) {
		const call = handler as (params: unknown) => unknown;
		senderValidatingHandle(name, (_e, params: unknown) => call(params));
	}
	senderValidatingHandle("watcherUnsubscribe", (_e, { subscriptionId }: WatcherUnsubscribeParams) =>
		WatcherManager.getInstance().removeSubscription(subscriptionId),
	);

	senderValidatingHandle("isFullScreen", (event) =>
		Promise.resolve(BrowserWindow.fromWebContents(event.sender)?.isFullScreen() ?? false),
	);

	senderValidatingHandle("pathJoin", (_e, ...paths: Array<string>) => path.join(...paths));

	senderValidatingHandle(
		"showNativeMenu",
		async (event, { items, position }: ShowNativeMenuParams) => {
			const window = BrowserWindow.fromWebContents(event.sender);
			if (!window) return null;

			let selectedItemId: string | null = null;
			const menu = Menu.buildFromTemplate(
				buildNativeMenuTemplate(items, (itemId) => {
					selectedItemId = itemId;
				}),
			);

			await new Promise<void>((resolve) => {
				menu.popup({
					window,
					x: Math.round(position.x),
					y: Math.round(position.y),
					callback: () => resolve(),
				});
			});

			return selectedItemId;
		},
	);

	senderValidatingHandle(
		"streamAiResponse",
		async (event, { requestId, systemMessage, prompt }: StreamAiResponseParams) =>
			sdk.streamAiResponse(systemMessage, prompt, (error, token) => {
				if (error) return;
				if (!event.sender.isDestroyed())
					event.sender.send("streamAiResponseToken", { requestId, token });
			}),
	);

	senderValidatingHandle("watcherSubscribe", async (event, { projectId }: WatcherSubscribeParams) =>
		WatcherManager.getInstance().subscribeToProject(projectId, event),
	);
};

/**
 * A `but://app/...` link, translated to whatever this build actually serves:
 * the dev server in development, our own scheme when packaged. Returns null
 * for anything that is not one of our links.
 */
const deepLinkTargetUrl = (link: string): string | null => {
	const url = newUrlOrNull(link);
	if (
		url === null ||
		url.protocol !== `${liteProtocolScheme}:` ||
		url.host !== liteProtocolHost ||
		url.pathname.includes("..")
	)
		return null;

	const devServerUrl = process.env.VITE_DEV_SERVER_URL;
	const base = new URL(devServerUrl ?? `${liteProtocolScheme}://${liteProtocolHost}/`);
	const target = new URL(`${url.pathname}${url.search}`, base);

	// The path decides the host when it starts with `//`, so the link's own host
	// having checked out says nothing about where this one points.
	if (target.protocol !== base.protocol || target.host !== base.host) return null;

	return target.href;
};

/**
 * Sign in from a `but://login?access_token=…` link, which is how the login page
 * hands the account back once it knows which client asked.
 */
const completeLogin = async (url: URL): Promise<boolean> => {
	if (url.host !== "login") return false;

	const accessToken = url.searchParams.get("access_token");
	if (accessToken === null) return true;

	try {
		await sdk.loginAndPersist(accessToken);
	} catch (error) {
		// oxlint-disable-next-line no-console
		console.error("Failed to sign in from a login link", error);
	}
	return true;
};

/**
 * Open a deep link in the window we already have, or start one if the app was
 * launched by the link. The project it names is checked by the route itself,
 * which covers every other way a URL arrives too.
 */
const openDeepLink = async (link: string): Promise<void> => {
	const url = newUrlOrNull(link);
	if (url !== null && url.protocol === `${liteProtocolScheme}:` && (await completeLogin(url))) {
		BrowserWindow.getAllWindows()[0]?.focus();
		return;
	}

	const target = deepLinkTargetUrl(link);
	if (target === null) {
		// oxlint-disable-next-line no-console
		console.error(`Ignored deep link ${link}`);
		return;
	}

	const [existing] = BrowserWindow.getAllWindows();
	if (!existing) {
		await createMainWindow(target);
		return;
	}

	if (existing.isMinimized()) existing.restore();
	existing.focus();
	await existing.loadURL(target);
};

/** The `but://` link in a launch argv, if the OS started us with one. */
const deepLinkFromArgv = (argv: Array<string>): string | undefined =>
	argv.find((arg) => arg.startsWith(`${liteProtocolScheme}://`));

const createMainWindow = async (initialUrl?: string): Promise<void> => {
	const icon = getWindowIcon();
	const mainWindow = new BrowserWindow({
		width: 1024,
		height: 768,
		show: !isHeadless,
		minWidth: 545,
		minHeight: 400,
		icon,
		titleBarStyle: process.platform === "darwin" ? "hidden" : "default",
		trafficLightPosition: process.platform === "darwin" ? { x: 16, y: 19 } : undefined,
		webPreferences: {
			contextIsolation: true,
			nodeIntegration: false,
			preload: path.join(currentDirPath, "preload.cjs"),
		},
	});

	const notifyFullScreenChange = () => {
		mainWindow.webContents.send("fullScreenChange", mainWindow.isFullScreen());
	};
	mainWindow.on("enter-full-screen", notifyFullScreenChange);
	mainWindow.on("leave-full-screen", notifyFullScreenChange);

	const devServerUrl = process.env.VITE_DEV_SERVER_URL;
	if (devServerUrl !== undefined) {
		await mainWindow.loadURL(initialUrl ?? devServerUrl);
		return;
	}

	const rootUrl = `${liteProtocolScheme}://${liteProtocolHost}/`;
	await mainWindow.loadURL(initialUrl ?? rootUrl);
	registerUpdater(mainWindow);
	checkForUpdates();
};

app.enableSandbox(); // forces sandboxing for all renderers, even if they try to launch without

// One instance owns the protocol: a second launch (how Windows and Linux
// deliver a link) hands its argv to the first and exits.
if (!app.requestSingleInstanceLock()) {
	app.quit();
} else {
	app.on("second-instance", (_event, argv) => {
		const link = deepLinkFromArgv(argv);
		if (link !== undefined) void openDeepLink(link);
	});

	// macOS delivers links here instead, both to a running app and to one the
	// link just launched.
	app.on("open-url", (event, url) => {
		event.preventDefault();
		void openDeepLink(url);
	});
}

void app.whenReady().then(async () => {
	applyGUISettings(await readSettings());
	await initApplicationNamespace(null);
	configureAskpass();

	if (app.isPackaged) {
		registerLiteProtocolHandler();

		// Basic non-Strict CSP based on https://cheatsheetseries.owasp.org/cheatsheets/Content_Security_Policy_Cheat_Sheet.html#basic-non-strict-csp-policy
		const productionCsp =
			"default-src 'none';" +
			"script-src 'self' 'wasm-unsafe-eval';" +
			"style-src 'self' 'unsafe-inline';" +
			"font-src 'self';" +
			"connect-src 'self';" +
			"object-src 'none';" +
			"base-uri 'none';" +
			"frame-ancestors 'none';" +
			"form-action 'none';" +
			// user-attachments assets on github.com 302 to GitHub's signed S3 bucket,
			// and CSP checks every redirect hop — hence the explicit bucket host.
			`img-src ${imgSrc};` +
			"worker-src 'self';";

		session.defaultSession.webRequest.onHeadersReceived((details, callback) => {
			callback({
				responseHeaders: {
					...details.responseHeaders,
					"Content-Security-Policy": [productionCsp],
				},
			});
		});
	} else {
		if (!isHeadless) {
			await installExtension([REACT_DEVELOPER_TOOLS, REDUX_DEVTOOLS]);

			const dockIcon = getMacDockIcon();
			if (dockIcon !== undefined) app.dock?.setIcon(dockIcon);
		}

		// Loose dev CSP to allow for hot reload and development tools. This could be tightened with
		// nonce-based CSP instead of using unsafe-inline, but it's just not worth the hassle right now.
		const developmentCsp =
			"default-src 'none';" +
			// unsafe-inline necessary for HMR. Potentially fixable with nonce-based Strict CSP.
			"script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval';" +
			// unsafe-inline necessary for HMR. Potentially fixable with nonce-based Strict CSP.
			"style-src 'self' 'unsafe-inline';" +
			"font-src 'self';" +
			// ws source for HMR
			"connect-src 'self' ws://127.0.0.1:5173;" +
			"object-src 'none';" +
			"base-uri 'none';" +
			"frame-ancestors 'none';" +
			"form-action 'none';" +
			// user-attachments assets on github.com 302 to GitHub's signed S3 bucket,
			// and CSP checks every redirect hop — hence the explicit bucket host.
			`img-src ${imgSrc};` +
			"worker-src 'self';";

		session.defaultSession.webRequest.onHeadersReceived((details, callback) => {
			// Skip extensions, or React dev tools don't work
			if (details.url.startsWith("chrome-extension://")) {
				callback({ responseHeaders: details.responseHeaders });
				return;
			}

			callback({
				responseHeaders: {
					...details.responseHeaders,
					"Content-Security-Policy": [developmentCsp],
				},
			});
		});
	}

	session.defaultSession.setPermissionRequestHandler((webContents, permission, callback) => {
		const url = newUrlOrNull(webContents.getURL());
		if (isTrustedLocalOrigin(url) && trustedOriginDefaultPermissions.includes(permission))
			return callback(true);

		// oxlint-disable-next-line no-console
		console.error(`Blocked permission request for ${permission} from ${url?.href ?? "<unknown>"}`);
		return callback(false);
	});

	registerIpcHandlers();

	// Dev runs from the electron binary, which needs to be told which program
	// and arguments to relaunch for a link.
	if (app.isPackaged) {
		app.setAsDefaultProtocolClient(liteProtocolScheme);
	} else {
		app.setAsDefaultProtocolClient(liteProtocolScheme, process.execPath, [
			path.resolve(process.argv[1] ?? ""),
		]);
	}

	const launchLink = deepLinkFromArgv(process.argv);
	// Windows and Linux deliver a cold-launch link only through argv, never as
	// `open-url`, so a login link arriving that way has to be handled here too.
	const launchUrl = launchLink === undefined ? null : newUrlOrNull(launchLink);
	if (launchUrl?.protocol === `${liteProtocolScheme}:`) await completeLogin(launchUrl);

	await createMainWindow(
		launchLink === undefined ? undefined : (deepLinkTargetUrl(launchLink) ?? undefined),
	);

	app.on("activate", () => {
		if (BrowserWindow.getAllWindows().length === 0) void createMainWindow();
	});
});

app.on("before-quit", () => {
	WatcherManager.getInstance().destroy();
});

app.on("window-all-closed", () => {
	WatcherManager.getInstance().destroy();
	if (process.platform !== "darwin") app.quit();
});

app.on("web-contents-created", (_, contents) => {
	contents.on("will-navigate", (event, navigationUrl) => {
		const targetUrl = newUrlOrNull(navigationUrl);
		// Where the user is lives in the URL, so opening a link to a branch or a
		// commit is an ordinary navigation. Anything off our origin stays blocked.
		if (isTrustedLocalOrigin(targetUrl)) return;

		// oxlint-disable-next-line no-console
		console.error(`Blocked navigation to ${navigationUrl}`);
		event.preventDefault();
	});

	contents.setWindowOpenHandler(({ url }) => {
		// oxlint-disable-next-line no-console
		console.error(`Blocked opening new window for ${url}`);
		return { action: "deny" };
	});

	contents.on("will-attach-webview", (event, webPreferences, _) => {
		// oxlint-disable-next-line no-console
		console.error(`Blocked attaching webview ${JSON.stringify(webPreferences)}`);
		event.preventDefault();
	});
});
