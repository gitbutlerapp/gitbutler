import { liteIpcChannels } from "#electron/ipc";
import {
	cancelLongRunningTask,
	listLongRunningTasks,
	startLongRunningTask,
	subscribeLongRunningTaskEvents,
} from "#electron/model/longRunning";
import { listProjects } from "#electron/model/projects";
import { headInfo } from "#electron/model/workspace";
import { app, BrowserWindow, ipcMain } from "electron";
import path from "node:path";
import { fileURLToPath } from "node:url";

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);

function registerIpcHandlers(): void {
	ipcMain.handle(liteIpcChannels.ping, async (_event, input: string): Promise<string> => {
		return await Promise.resolve(`pong: ${input}`);
	});

	ipcMain.handle(liteIpcChannels.getVersion, async (): Promise<string> => {
		return await Promise.resolve(app.getVersion());
	});

	// Returns all known task snapshots for initial renderer hydration.
	ipcMain.handle(
		liteIpcChannels.listLongRunningTasks,
		async (): Promise<ReturnType<typeof listLongRunningTasks>> => {
			return await Promise.resolve(listLongRunningTasks());
		},
	);

	// Starts a new non-blocking task in Rust and returns its task id.
	ipcMain.handle(
		liteIpcChannels.startLongRunningTask,
		async (_event, durationMs: number): Promise<number> => {
			return await Promise.resolve(startLongRunningTask(durationMs));
		},
	);

	// Requests cancellation for an existing task id.
	ipcMain.handle(
		liteIpcChannels.cancelLongRunningTask,
		async (_event, taskId: number): Promise<boolean> => {
			return await Promise.resolve(cancelLongRunningTask(taskId));
		},
	);

	// Pushes incremental task snapshot updates from main to all renderer windows.
	subscribeLongRunningTaskEvents((event) => {
		for (const browserWindow of BrowserWindow.getAllWindows()) {
			browserWindow.webContents.send(liteIpcChannels.longRunningTaskEvent, event);
		}
	});
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
	if (devServerUrl) {
		await mainWindow.loadURL(devServerUrl);
		mainWindow.webContents.openDevTools({ mode: "detach" });
		return;
	}

	await mainWindow.loadFile(path.join(currentDirPath, "../ui/index.html"));
}

app.whenReady().then(async () => {
	registerIpcHandlers();
	await createMainWindow();

	app.on("activate", () => {
		if (BrowserWindow.getAllWindows().length === 0) {
			void createMainWindow();
		}
	});
});

app.on("window-all-closed", () => {
	if (process.platform !== "darwin") {
		app.quit();
	}
});

ipcMain.handle(liteIpcChannels.listProjects, async () => {
	return await listProjects();
});
ipcMain.handle(liteIpcChannels.headInfo, async (_e, projectId: string) => {
	return await headInfo(projectId);
});
