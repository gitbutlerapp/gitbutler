import { handleErrorWithSentry } from '@sentry/sveltekit';
import { error as logErrorToFile } from 'tauri-plugin-log-api';
import type { HandleClientError } from '@sveltejs/kit';

const errorHandler: HandleClientError = ({ error, message }) => {
	const errorId = crypto.randomUUID();

	logErrorToFile(message);
	console.error(`${errorId}: ${message}\n${(error as Error)?.stack}`);

	return {
		message,
		errorId
	};
};

export const handleError = handleErrorWithSentry(errorHandler);
