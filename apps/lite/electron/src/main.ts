import { checkForUpdates, registerUpdater } from "./updater.js";
import WatcherManager from "./watcher.js";
import {
	liteIpcChannels,
	type AbsorbParams,
	type AbsorptionPlanParams,
	type AssignHunkParams,
	type BranchDetailsParams,
	type BranchDiffParams,
	type CommitAmendParams,
	type CommitCreateParams,
	type CommitDiscardParams,
	type CommitDetailsWithLineStatsParams,
	type CommitInsertBlankParams,
	type CommitMoveParams,
	type CommitSquashParams,
	type CommitRewordParams,
	type CommitMoveChangesBetweenParams,
	type CommitUncommitChangesParams,
	type MoveBranchParams,
	type PushStackLegacyParams,
	type TearOffBranchParams,
	type TreeChangeDiffParams,
	type UpdateBranchNameParams,
	type ApplyParams,
	type ShowNativeMenuParams,
	type UnapplyStackParams,
	WatcherSubscribeParams,
	WatcherUnsubscribeParams,
	NativeMenuPopupItem,
	CommitUncommitParams,
} from "./ipc.js";
import {
	absorb,
	absorptionPlan,
	apply,
	assignHunk,
	branchDetails,
	branchDiff,
	changesInWorktree,
	commitAmend,
	commitCreate,
	commitDiscard,
	commitInsertBlank,
	commitSquash,
	commitReword,
	commitUncommitChanges,
	headInfo,
	commitMove,
	commitDetailsWithLineStats,
	commitMoveChangesBetween,
	listBranches,
	listProjectsStateless,
	moveBranch,
	pushStackLegacy,
	tearOffBranch,
	treeChangeDiffs,
	unapplyStack,
	updateBranchName,
	BranchListingFilter,
	commitUncommit,
} from "@gitbutler/but-sdk";
import {
	app,
	BrowserWindow,
	ipcMain,
	Menu,
	net,
	protocol,
	session,
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

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);

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

const liteProtocolScheme = "lite";
const liteProtocolHost = "app";
const contentRootURL = pathToFileURL(path.join(currentDirPath, "../ui"));

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
		if (pathname.includes("..") || host !== liteProtocolHost)
			return new Response("Not found", {
				status: 404,
				headers: { "content-type": "text/html" },
			});

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

// Dev-only runtime icons path (packaged builds rely on electron-builder icons).
const iconsPath = path.join(currentDirPath, "../../resources/icons");

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

	senderValidatingHandle(
		liteIpcChannels.absorptionPlan,
		(_e, { projectId, target }: AbsorptionPlanParams) => absorptionPlan(projectId, target),
	);
	senderValidatingHandle(
		liteIpcChannels.absorb,
		(_e, { projectId, absorptionPlan }: AbsorbParams) => absorb(projectId, absorptionPlan),
	);
	senderValidatingHandle(liteIpcChannels.apply, (_e, { projectId, existingBranch }: ApplyParams) =>
		apply(projectId, existingBranch),
	);
	senderValidatingHandle(
		liteIpcChannels.assignHunk,
		(_e, { projectId, assignments }: AssignHunkParams) => assignHunk(projectId, assignments),
	);
	senderValidatingHandle(
		liteIpcChannels.branchDetails,
		(_e, { projectId, branchName, remote }: BranchDetailsParams) =>
			branchDetails(projectId, branchName, remote),
	);
	senderValidatingHandle(
		liteIpcChannels.branchDiff,
		(_e, { projectId, branch }: BranchDiffParams) => branchDiff(projectId, branch),
	);
	senderValidatingHandle(liteIpcChannels.changesInWorktree, (_e, projectId: string) =>
		changesInWorktree(projectId),
	);
	senderValidatingHandle(
		liteIpcChannels.commitAmend,
		(_e, { projectId, commitId, changes, dryRun }: CommitAmendParams) =>
			commitAmend(projectId, commitId, changes, dryRun),
	);
	senderValidatingHandle(
		liteIpcChannels.commitCreate,
		(_e, { projectId, relativeTo, side, changes, message, dryRun }: CommitCreateParams) =>
			commitCreate(projectId, relativeTo, side, changes, message, dryRun),
	);
	senderValidatingHandle(
		liteIpcChannels.commitDiscard,
		(_e, { projectId, subjectCommitId, dryRun }: CommitDiscardParams) =>
			commitDiscard(projectId, subjectCommitId, dryRun),
	);
	senderValidatingHandle(
		liteIpcChannels.commitDetailsWithLineStats,
		(_e, { projectId, commitId }: CommitDetailsWithLineStatsParams) =>
			commitDetailsWithLineStats(projectId, commitId),
	);
	senderValidatingHandle(
		liteIpcChannels.commitInsertBlank,
		(_e, { projectId, relativeTo, side, dryRun }: CommitInsertBlankParams) =>
			commitInsertBlank(projectId, relativeTo, side, dryRun),
	);
	senderValidatingHandle(
		liteIpcChannels.commitMove,
		(_e, { projectId, subjectCommitIds, relativeTo, side, dryRun }: CommitMoveParams) =>
			commitMove(projectId, subjectCommitIds, relativeTo, side, dryRun),
	);
	senderValidatingHandle(
		liteIpcChannels.commitSquash,
		(_e, { projectId, sourceCommitIds, destinationCommitId, dryRun }: CommitSquashParams) =>
			commitSquash(projectId, sourceCommitIds, destinationCommitId, "KeepBoth", dryRun),
	);
	senderValidatingHandle(
		liteIpcChannels.commitReword,
		(_e, { projectId, commitId, message, dryRun }: CommitRewordParams) =>
			commitReword(projectId, commitId, message, dryRun),
	);
	senderValidatingHandle(
		liteIpcChannels.commitMoveChangesBetween,
		(
			_e,
			{
				projectId,
				sourceCommitId,
				destinationCommitId,
				changes,
				dryRun,
			}: CommitMoveChangesBetweenParams,
		) => commitMoveChangesBetween(projectId, sourceCommitId, destinationCommitId, changes, dryRun),
	);
	senderValidatingHandle(
		liteIpcChannels.commitUncommit,
		(_e, { projectId, subjectCommitIds: commitIds, assignTo, dryRun }: CommitUncommitParams) =>
			commitUncommit(projectId, commitIds, assignTo, dryRun),
	);
	senderValidatingHandle(
		liteIpcChannels.commitUncommitChanges,
		(_e, { projectId, commitId, changes, assignTo, dryRun }: CommitUncommitChangesParams) =>
			commitUncommitChanges(projectId, commitId, changes, assignTo, dryRun),
	);
	senderValidatingHandle(liteIpcChannels.getVersion, () => Promise.resolve(app.getVersion()));
	senderValidatingHandle(liteIpcChannels.headInfo, (_e, projectId: string) => headInfo(projectId));
	senderValidatingHandle(
		liteIpcChannels.listBranches,
		(_e, projectId: string, filter: BranchListingFilter | null) => listBranches(projectId, filter),
	);
	senderValidatingHandle(liteIpcChannels.listProjects, () => listProjectsStateless());
	senderValidatingHandle(
		liteIpcChannels.moveBranch,
		(_e, { projectId, subjectBranch, targetBranch, dryRun }: MoveBranchParams) =>
			moveBranch(projectId, subjectBranch, targetBranch, dryRun),
	);
	senderValidatingHandle(
		liteIpcChannels.updateBranchName,
		(_e, { projectId, stackId, branchName, newName }: UpdateBranchNameParams) =>
			updateBranchName(projectId, stackId, branchName, newName),
	);
	senderValidatingHandle(
		liteIpcChannels.tearOffBranch,
		(_e, { projectId, subjectBranch, dryRun }: TearOffBranchParams) =>
			tearOffBranch(projectId, subjectBranch, dryRun),
	);
	senderValidatingHandle(liteIpcChannels.ping, (_event, input: string) =>
		Promise.resolve(`pong: ${input}`),
	);
	senderValidatingHandle(
		liteIpcChannels.pushStackLegacy,
		(_e, { projectId, stackId, branch }: PushStackLegacyParams) =>
			pushStackLegacy(projectId, stackId, false, false, branch, true),
	);
	senderValidatingHandle(
		liteIpcChannels.showNativeMenu,
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
		liteIpcChannels.treeChangeDiffs,
		(_e, { projectId, change }: TreeChangeDiffParams) => treeChangeDiffs(projectId, change),
	);
	senderValidatingHandle(
		liteIpcChannels.unapplyStack,
		(_e, { projectId, stackId }: UnapplyStackParams) => unapplyStack(projectId, stackId),
	);
	senderValidatingHandle(
		liteIpcChannels.watcherSubscribe,
		async (event, { projectId }: WatcherSubscribeParams) =>
			WatcherManager.getInstance().subscribeToProject(projectId, event),
	);
	senderValidatingHandle(
		liteIpcChannels.watcherUnsubscribe,
		(_e, { subscriptionId }: WatcherUnsubscribeParams) =>
			WatcherManager.getInstance().removeSubscription(subscriptionId),
	);
	senderValidatingHandle(liteIpcChannels.watcherStopAll, () =>
		WatcherManager.getInstance().stopAllWatchersForShutdown(),
	);
};

const createMainWindow = async (): Promise<void> => {
	const icon = getWindowIcon();
	const mainWindow = new BrowserWindow({
		width: 1024,
		height: 768,
		icon,
		webPreferences: {
			contextIsolation: true,
			nodeIntegration: false,
			preload: path.join(currentDirPath, "preload.cjs"),
		},
	});

	const devServerUrl = process.env.VITE_DEV_SERVER_URL;
	if (devServerUrl !== undefined) {
		await mainWindow.loadURL(devServerUrl);
		return;
	}

	const rootUrl = `${liteProtocolScheme}://${liteProtocolHost}/`;
	await mainWindow.loadURL(rootUrl);
	registerUpdater(mainWindow);
	checkForUpdates();
};

app.enableSandbox(); // forces sandboxing for all renderers, even if they try to launch without
void app.whenReady().then(async () => {
	if (app.isPackaged) {
		registerLiteProtocolHandler();

		// Basic non-Strict CSP based on https://cheatsheetseries.owasp.org/cheatsheets/Content_Security_Policy_Cheat_Sheet.html#basic-non-strict-csp-policy
		const productionCsp =
			"default-src 'none';" +
			"script-src 'self' 'wasm-unsafe-eval';" +
			// Hash is for inline style in index.html
			"style-src 'self' 'sha256-XBXaUBQCe+0UGd1QCfoPFCc7UsLKd8xrn9oXNYqjFog=';" +
			// react-resizable-panels has inline styles in elements. `style-src-attr 'unsafe-inline'` is slightly more narrow
			// than just `style-src 'unsafe-inline'`, but we should still try to get rid of this.
			"style-src-attr 'unsafe-inline';" +
			"font-src 'self';" +
			"connect-src 'self';" +
			"object-src 'none';" +
			"base-uri 'none';" +
			"frame-ancestors 'none';" +
			"form-action 'none';" +
			"img-src 'self' data:;" +
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
		await installExtension([REACT_DEVELOPER_TOOLS, REDUX_DEVTOOLS]);

		if (process.platform === "darwin") {
			const dockIcon = getMacDockIcon();
			if (dockIcon !== undefined && app.dock) app.dock.setIcon(dockIcon);
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
			"img-src 'self' data:;" +
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
	await createMainWindow();

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
