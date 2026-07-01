import { logErrorToFile } from "$lib/backend";
import { logError, setLogErrorToFile } from "$lib/error/logError";
import { polyfillAbortSignalTimeout } from "$lib/polyfills/abortSignal";
import type { HandleClientError } from "@sveltejs/kit";

// Apply polyfills before any code runs
polyfillAbortSignalTimeout();

// Wire up backend file logger for error handling.
setLogErrorToFile(logErrorToFile);

// SvelteKit error handler.
export function handleError({
	error,
	status,
}: {
	error: unknown;
	status: number;
}): ReturnType<HandleClientError> {
	if (status !== 404) {
		logError(error);
	}
	return {
		message: String(error),
	};
}

// Handler for unhandled errors inside promises.
window.onunhandledrejection = (e: PromiseRejectionEvent) => {
	e.preventDefault(); // Suppresses default console logger.
	logError(e);
};
