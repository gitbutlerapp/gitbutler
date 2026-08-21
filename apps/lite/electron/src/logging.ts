import { initTracing, shutdownTracing } from "@gitbutler/but-sdk";
import { app } from "electron";
import log from "electron-log/main";
import type { LevelOption } from "electron-log";
import path from "node:path";

// LOG_LEVEL also controls the Rust tracing verbosity, so one knob covers both files.
const fileLevels: Record<string, LevelOption> = {
	error: "error",
	warn: "warn",
	info: "info",
	debug: "debug",
	trace: "debug",
	off: false,
};

/**
 * Route all logging into the app's log folder: Rust backend tracing into daily
 * `GitButler.<date>.log` files, and main-process + renderer console output into
 * electron-log's `main.log`.
 */
export const initLogging = (): void => {
	try {
		// The renderer never imports electron-log, so skip its preload injection and
		// capture the renderer's console output from the outside instead.
		log.initialize({ preload: false, spyRendererConsole: true });
		log.transports.file.resolvePathFn = (variables) =>
			path.join(app.getPath("logs"), variables.fileName ?? "main.log");
		log.transports.file.level = fileLevels[(process.env.LOG_LEVEL ?? "").toLowerCase()] ?? "info";
		// Log uncaught exceptions and unhandled rejections in the main process, keeping
		// Electron's error dialog for uncaught exceptions.
		log.errorHandler.startCatching();
		// Route the main process's own console calls through electron-log so they
		// persist to main.log too (they still echo to the terminal).
		Object.assign(console, log.functions);

		// The Rust side resolves the per-platform log folder for our bundle identifier
		// (the electron-builder appId in package.json, with the desktop app's `.dev`
		// channel convention for unpackaged builds) and installs the tracing
		// subscriber; electron-log then writes to the same folder.
		app.setAppLogsPath(
			initTracing(
				app.isPackaged ? "com.gitbutler.lite" : "com.gitbutler.lite.dev",
				!app.isPackaged,
			),
		);
		// Drain the buffered log writer on exit so the last lines aren't lost.
		app.on("will-quit", shutdownTracing);
	} catch (error) {
		// The app must still start when logging cannot.
		// oxlint-disable-next-line no-console
		console.error("Failed to initialize logging", error);
	}
};
