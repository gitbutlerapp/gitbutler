import { app, BrowserWindow, ipcMain } from 'electron';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { liteIpcChannels } from '#electron/ipc';
import { listProjects } from '#electron/model/projects';

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);

function registerIpcHandlers(): void {
	ipcMain.handle(liteIpcChannels.ping, async (_event, input: string): Promise<string> => {
		return await Promise.resolve(`pong: ${input}`);
	});

	ipcMain.handle(liteIpcChannels.getVersion, async (): Promise<string> => {
		return await Promise.resolve(app.getVersion());
	});
}

async function createMainWindow(): Promise<void> {
	const mainWindow = new BrowserWindow({
		width: 1024,
		height: 768,
		webPreferences: {
			contextIsolation: true,
			nodeIntegration: false,
			preload: path.join(currentDirPath, 'preload.cjs')
		}
	});

	const devServerUrl = process.env.VITE_DEV_SERVER_URL;
	if (devServerUrl) {
		await mainWindow.loadURL(devServerUrl);
		mainWindow.webContents.openDevTools({ mode: 'detach' });
		return;
	}

	await mainWindow.loadFile(path.join(currentDirPath, '../ui/index.html'));
}

app.whenReady().then(async () => {
	registerIpcHandlers();
	await createMainWindow();

	app.on('activate', () => {
		if (BrowserWindow.getAllWindows().length === 0) {
			void createMainWindow();
		}
	});
});

app.on('window-all-closed', () => {
	if (process.platform !== 'darwin') {
		app.quit();
	}
});

ipcMain.handle(liteIpcChannels.listProjects, () => {
	return listProjects();
});
