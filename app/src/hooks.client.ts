import { showError } from '$lib/notifications/toasts';
import { captureException } from '@sentry/sveltekit';
import { error as logErrorToFile } from 'tauri-plugin-log-api';
import type { HandleClientError } from '@sveltejs/kit';

// SvelteKit error handler.
export function handleError({
	error,
	status
}: {
	error: any;
	status: number;
}): ReturnType<HandleClientError> {
	if (status !== 404) {
		logError(error);
	}
	return {
		message: error.message ?? error.toString()
	};
}

// Handler for unhandled errors inside promises.
window.onunhandledrejection = (e: PromiseRejectionEvent) => {
	logError(e.reason);
};

function logError(err: any) {
	let message = err instanceof Error ? err.message : err.toString();
	const stack = err instanceof Error ? err.stack : undefined;

	const id = captureException(err, {
		mechanism: {
			type: 'sveltekit',
			handled: false
		}
	});
	message = `${id}: ${message}\n`;
	if (stack) message = `${message}\n${stack}\n`;

	logErrorToFile(message);
	console.error(message);
	showError('Something went wrong', err);
	return id;
}
