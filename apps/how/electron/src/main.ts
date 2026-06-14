import { HowService } from "./how-service.js";
import { howIpcChannels, type OpenProjectResult } from "./ipc.js";
import { createLogger, type Logger } from "./logger.js";
import { normalizeProjectSettings } from "./settings.js";
import {
	app,
	BrowserWindow,
	dialog,
	ipcMain,
	net,
	protocol,
	session,
	type IpcMainInvokeEvent,
} from "electron";
import { REACT_DEVELOPER_TOOLS, installExtension } from "electron-devtools-installer";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);
const howProtocolScheme = "how";
const howProtocolHost = "app";
const contentRootURL = pathToFileURL(path.join(currentDirPath, "../ui"));
const iconsPath = path.join(currentDirPath, "../../resources/icons");

protocol.registerSchemesAsPrivileged([
	{
		scheme: howProtocolScheme,
		privileges: {
			standard: true,
			secure: true,
			supportFetchAPI: true,
		},
	},
]);

let mainWindow: BrowserWindow | null = null;
let service: HowService | null = null;
let logger: Logger | null = null;

function configureUserDataPath(): void {
	const userDataPath = process.env.HOW_E2E_USER_DATA_DIR;
	if (!userDataPath) return;
	fs.mkdirSync(userDataPath, { recursive: true });
	app.setPath("userData", userDataPath);
}

function windows(): Array<BrowserWindow> {
	return BrowserWindow.getAllWindows();
}

function getService(): HowService {
	if (!service) throw new Error("How is not ready yet.");
	return service;
}

function getLogger(): Logger {
	if (!logger) throw new Error("How logging is not ready yet.");
	return logger;
}

function registerProtocolHandler(): void {
	protocol.handle(howProtocolScheme, async (req) => {
		const { host, pathname } = new URL(req.url);
		if (pathname.includes("..") || host !== howProtocolHost)
			return new Response("Not found", {
				status: 404,
				headers: { "content-type": "text/html" },
			});

		const urlToServe = new URL(contentRootURL);
		urlToServe.pathname += pathname.startsWith("/assets/") ? pathname : "/index.html";
		return await net.fetch(urlToServe.toString());
	});
}

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

async function createMainWindow(): Promise<void> {
	mainWindow = new BrowserWindow({
		show: process.env.HOW_E2E_HEADLESS !== "1",
		width: 920,
		height: 680,
		minWidth: 720,
		minHeight: 520,
		backgroundColor: "#f8f7f2",
		icon: getWindowIcon(),
		webPreferences: {
			preload: path.join(currentDirPath, "preload.cjs"),
			nodeIntegration: false,
			contextIsolation: true,
		},
	});

	mainWindow.once("ready-to-show", () => {
		if (process.env.HOW_E2E_HEADLESS !== "1") mainWindow?.show();
	});

	if (process.env.VITE_DEV_SERVER_URL) await mainWindow.loadURL(process.env.VITE_DEV_SERVER_URL);
	else await mainWindow.loadURL(`${howProtocolScheme}://${howProtocolHost}/`);

	mainWindow.on("closed", () => {
		mainWindow = null;
	});
}

function validateSender(event: IpcMainInvokeEvent): void {
	const url = event.senderFrame?.url;
	if (!url) throw new Error("How could not verify this window.");
	if (process.env.VITE_DEV_SERVER_URL) {
		if (url.startsWith(process.env.VITE_DEV_SERVER_URL)) return;
	} else if (url.startsWith(`${howProtocolScheme}://${howProtocolHost}/`)) return;
	throw new Error("How blocked a request from an unknown window.");
}

function handle<Args extends Array<unknown>, Return>(
	channel: string,
	listener: (event: IpcMainInvokeEvent, ...args: Args) => Promise<Return> | Return,
): void {
	ipcMain.handle(channel, async (event, ...args: Args) => {
		validateSender(event);
		return await listener(event, ...args);
	});
}

async function selectProjectDirectory(mode: "open" | "start"): Promise<string | null> {
	if (process.env.HOW_E2E_PROJECT_PATH) {
		getLogger().info("Using e2e project directory", {
			mode,
			selectedPath: process.env.HOW_E2E_PROJECT_PATH,
		});
		return process.env.HOW_E2E_PROJECT_PATH;
	}

	getLogger().info("Opening project directory picker", { mode });
	const options: Electron.OpenDialogOptions = {
		title: mode === "open" ? "Open project" : "Start project",
		properties: mode === "start" ? ["openDirectory", "createDirectory"] : ["openDirectory"],
	};
	const result = mainWindow
		? await dialog.showOpenDialog(mainWindow, options)
		: await dialog.showOpenDialog(options);
	if (result.canceled) {
		getLogger().info("Project directory picker cancelled", { mode });
		return null;
	}
	const selectedPath = result.filePaths[0] ?? null;
	getLogger().info("Project directory picker selected path", { mode, selectedPath });
	return selectedPath;
}

function registerIpc(): void {
	handle(howIpcChannels.getStatus, async () => getService().getStatus());
	handle(howIpcChannels.openProject, async (): Promise<OpenProjectResult> => {
		const selectedPath = await selectProjectDirectory("open");
		if (!selectedPath) return { type: "cancelled" };
		try {
			const status = await getService().openProjectFromPath(selectedPath);
			return { type: "opened", status };
		} catch (error) {
			getLogger().error("Open project failed", error, { selectedPath });
			throw error;
		}
	});
	handle(howIpcChannels.startProject, async (): Promise<OpenProjectResult> => {
		const selectedPath = await selectProjectDirectory("start");
		if (!selectedPath) return { type: "cancelled" };
		try {
			const status = await getService().startProjectAtPath(selectedPath);
			return { type: "opened", status };
		} catch (error) {
			getLogger().error("Start project failed", error, { selectedPath });
			throw error;
		}
	});
	handle(howIpcChannels.deleteProject, async () => await getService().deleteProject());
	handle(howIpcChannels.createCheckpointNow, async () => await getService().createCheckpointNow());
	handle(howIpcChannels.publishProject, async (_event, input) => {
		const publishMode =
			typeof input === "object" &&
			input !== null &&
			"publishMode" in input &&
			input.publishMode === "direct"
				? "direct"
				: undefined;
		const destinationUrl =
			typeof input === "object" &&
			input !== null &&
			"destinationUrl" in input &&
			typeof input.destinationUrl === "string"
				? input.destinationUrl
				: undefined;
		return await getService().publishProject({ publishMode, destinationUrl });
	});
	handle(howIpcChannels.saveProjectSettings, async (_event, settings) => {
		if (typeof settings !== "object" || settings === null)
			throw new Error("How could not save settings.");
		return await getService().saveProjectSettings(normalizeProjectSettings(settings));
	});
	handle(howIpcChannels.viewCheckpoint, async (_event, checkpointId, options) => {
		if (typeof checkpointId !== "string" || checkpointId.length === 0)
			throw new Error("How could not find that checkpoint.");
		const discardBrowsingChanges =
			typeof options === "object" &&
			options !== null &&
			"discardBrowsingChanges" in options &&
			options.discardBrowsingChanges === true;
		return await getService().viewCheckpoint(checkpointId, { discardBrowsingChanges });
	});
	handle(
		howIpcChannels.continueFromCheckpoint,
		async () => await getService().continueFromCheckpoint(),
	);
	handle(howIpcChannels.returnToLatest, async (_event, options) => {
		const discardBrowsingChanges =
			typeof options === "object" &&
			options !== null &&
			"discardBrowsingChanges" in options &&
			options.discardBrowsingChanges === true;
		return await getService().returnToLatest({ discardBrowsingChanges });
	});
}

configureUserDataPath();

app.whenReady().then(async () => {
	logger = createLogger(path.join(app.getPath("userData"), "how.log"));
	logger.info("How app ready", {
		userData: app.getPath("userData"),
		devServerUrl: process.env.VITE_DEV_SERVER_URL ?? null,
		cwd: process.cwd(),
	});
	registerProtocolHandler();
	registerIpc();

	service = new HowService(path.join(app.getPath("userData"), "state.json"), windows, logger);
	await service.initialize();
	await createMainWindow();

	if (!app.isPackaged) {
		try {
			await installExtension(REACT_DEVELOPER_TOOLS);
		} catch (error) {
			// oxlint-disable-next-line no-console
			console.warn("Could not install React Developer Tools", error);
		}
	}

	session.defaultSession.setPermissionRequestHandler((_webContents, _permission, callback) => {
		callback(false);
	});

	app.on("activate", () => {
		if (BrowserWindow.getAllWindows().length === 0) void createMainWindow();
	});
});

app.on("before-quit", () => {
	void service?.stop();
});

app.on("window-all-closed", () => {
	if (process.platform !== "darwin") app.quit();
});
