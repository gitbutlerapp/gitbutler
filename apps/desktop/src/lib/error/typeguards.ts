import type { HttpError, BackendError } from './error';

export function isBackendError(err: unknown): err is BackendError {
	return (
		typeof err === 'object' &&
		err !== null &&
		'message' in err &&
		typeof err.message === 'string' &&
		'code' in err &&
		typeof err.code === 'string'
	);
}

export function isHttpError(err: unknown): err is HttpError {
	return (
		typeof err === 'object' &&
		err !== null &&
		'message' in err &&
		typeof err.message === 'string' &&
		'code' in err &&
		typeof err.code === 'string'
	);
}
