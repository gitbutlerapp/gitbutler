import { captureException } from '@sentry/sveltekit';
import { error as logErrorToFile } from 'tauri-plugin-log-api';
import type { HandleClientError } from '@sveltejs/kit';

export function handleError({
	error,
	status
}: {
	error: unknown;
	status: number;
}): ReturnType<HandleClientError> {
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
}
