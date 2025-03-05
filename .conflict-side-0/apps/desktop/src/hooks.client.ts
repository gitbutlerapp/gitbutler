import { showError } from '$lib/notifications/toasts';
import { captureException } from '@sentry/sveltekit';
import { error as logErrorToFile } from '@tauri-apps/plugin-log';
import type { HandleClientError } from '@sveltejs/kit';

// SvelteKit error handler.
export function handleError({
	error,
	status
}: {
	error: unknown;
	status: number;
}): ReturnType<HandleClientError> {
	if (status !== 404) {
		logError(error);
	}
	return {
		message: String(error)
	};
}

// Handler for unhandled errors inside promises.
window.onunhandledrejection = (e: PromiseRejectionEvent) => {
	logError(e.reason);
};

function logError(error: unknown) {
	try {
		captureException(error, {
			mechanism: {
				type: 'sveltekit',
				handled: false
			}
		});
		showError('Unhandled exception', error);
		logErrorToFile(String(error));
	} catch (err: unknown) {
		console.error('Error while trying to log error.', err);
	}
}
