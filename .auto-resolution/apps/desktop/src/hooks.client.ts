import { logErrorToFile } from '$lib/backend';
import { SilentError } from '$lib/error/error';
import { parseError } from '$lib/error/parser';
import { showError } from '$lib/notifications/toasts';
import { captureException } from '@sentry/sveltekit';
import type { HandleClientError } from '@sveltejs/kit';

const E2E_MESSAGES_TO_IGNORE = ['Unable to autolaunch a dbus-daemon without a $DISPLAY for X11'];

function shouldIgnoreError(error: unknown): boolean {
	if (import.meta.env.VITE_E2E === 'true') {
		const { message } = parseError(error);
		return E2E_MESSAGES_TO_IGNORE.includes(message);
	}
	return false;
}

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

function loggableError(error: unknown): string {
	if (error instanceof Error) {
		return error.message;
	}

	if (typeof error === 'string') {
		return error;
	}
	if (typeof error === 'object' && error !== null) {
		if ('message' in error && typeof error.message === 'string') {
			return error.message;
		}
		return JSON.stringify(error);
	}
	return String(error);
}

// Handler for unhandled errors inside promises.
window.onunhandledrejection = (e: PromiseRejectionEvent) => {
	e.preventDefault(); // Suppresses default console logger.
	logError(e);
};

function logError(error: unknown) {
	if (shouldIgnoreError(error)) {
		return;
	}

	try {
		captureException(error, {
			mechanism: {
				type: 'sveltekit',
				handled: false
			}
		});

		// Unwrap error if it's an unhandled promise rejection.
		if (error instanceof PromiseRejectionEvent) {
			error = error.reason;
		}

		if (!(error instanceof SilentError)) {
			showError('Unhandled exception', error);
		}

		const logMessage = loggableError(error);
		logErrorToFile(logMessage);

		console.error(error);
	} catch (err: unknown) {
		console.error('Error while trying to log error.', err);
	}
}
