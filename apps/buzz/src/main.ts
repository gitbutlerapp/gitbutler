import { app, BrowserWindow } from 'electron';

function createWindow() {
	const win = new BrowserWindow({
		width: 800,
		height: 600
	});

	if (process.env.ELECTRON_ENV === 'development') {
		win.loadURL('http://localhost:1420');
	} else {
		// TODO: Some bundled version
		win.loadFile('index.html');
	}
}

app.whenReady().then(() => {
	createWindow();

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
