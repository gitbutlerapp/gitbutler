import { liteIpcChannels } from "#electron/ipc";
import { headInfoNapi, listProjectsStatelessNapi } from "@gitbutler/but-sdk";
import { app, BrowserWindow, ipcMain } from "electron";
import { REACT_DEVELOPER_TOOLS, installExtension } from "electron-devtools-installer";
import path from "node:path";
import { fileURLToPath } from "node:url";

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);

function registerIpcHandlers(): void {
	ipcMain.handle(liteIpcChannels.ping, (_event, input: string) =>
		Promise.resolve(`pong: ${input}`),
	);
	ipcMain.handle(liteIpcChannels.getVersion, () => Promise.resolve(app.getVersion()));
	ipcMain.handle(liteIpcChannels.listProjects,  () => listProjectsStatelessNapi());
	ipcMain.handle(liteIpcChannels.headInfo, (_e, projectId: string) => headInfoNapi(projectId));
}

async function createMainWindow(): Promise<void> {
	const mainWindow = new BrowserWindow({
		width: 1024,
		height: 768,
		webPreferences: {
			contextIsolation: true,
			nodeIntegration: false,
			preload: path.join(currentDirPath, "preload.cjs"),
		},
	});

	const devServerUrl = process.env.VITE_DEV_SERVER_URL;
	if (devServerUrl !== undefined) {
		await mainWindow.loadURL(devServerUrl);
		mainWindow.webContents.openDevTools({ mode: "detach" });
		return;
	}

	await mainWindow.loadFile(path.join(currentDirPath, "../ui/index.html"));
}

void app.whenReady().then(async () => {
	// TODO: if (!app.isPackaged)
	await installExtension(REACT_DEVELOPER_TOOLS);
	registerIpcHandlers();
	await createMainWindow();

	app.on("activate", () => {
		if (BrowserWindow.getAllWindows().length === 0) void createMainWindow();
	});
});

app.on("window-all-closed", () => {
	if (process.platform !== "darwin") app.quit();
});
