import { app, BrowserWindow, dialog, ipcMain } from 'electron';
import path from 'path';

function createWindow() {
	const win = new BrowserWindow({
		width: 800,
		height: 600,
		webPreferences: {
			preload: path.join(__dirname, 'preload.js')
		}
	});

	if (process.env.ELECTRON_ENV === 'development') {
		win.loadURL(`http://${process.env.VITE_HOST}:${process.env.VITE_PORT}`);
	} else {
		// TODO: Some bundled version
		win.loadFile('index.html');
	}
}

app.whenReady().then(() => {
	createWindow();

	ipcMain.handle('dialog:openDirectory', openDirectory);

	app.on('activate', () => {
		if (BrowserWindow.getAllWindows().length === 0) {
			createWindow();
		}
	});
});

app.on('window-all-closed', () => {
	if (process.platform !== 'darwin') {
		app.quit();
	}
});

async function openDirectory() {
	const { canceled, filePaths } = await dialog.showOpenDialog({ properties: ['openDirectory'] });

	if (!canceled) return filePaths.at(0);
}
