import { captureException } from '@sentry/sveltekit';
import { error as logErrorToFile } from 'tauri-plugin-log-api';
import type { HandleClientError } from '@sveltejs/kit';

// eslint-disable-next-line func-style
export const handleError: HandleClientError = ({ error, status }) => {
	let errorId: string = crypto.randomUUID();

	if (status !== 404) {
		errorId = captureException(error, {
			mechanism: {
				type: 'sveltekit',
				handled: false
			}
		});
	}

	logErrorToFile((error as Error).message);
	console.error(`${errorId}: ${(error as Error).message}\n${(error as Error)?.stack}`);

	return {
		message: (error as Error).message,
		errorId
	};
};
