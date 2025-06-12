import type { HttpError, BackendError, UnhandledPromiseError } from '$lib/error/error';

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

export function isPromiseRejection(err: unknown): err is UnhandledPromiseError {
	return (
		typeof err === 'object' && err !== null && 'reason' in err && typeof err.reason === 'object'
	);
}

export function isHttpError(err: unknown): err is HttpError {
	return (
		typeof err === 'object' &&
		err !== null &&
		'message' in err &&
		typeof err.message === 'string' &&
		'status' in err &&
		typeof err.status === 'number'
	);
}

export function isReduxActionError(err: unknown): err is {
	type: string;
	payload: string;
	error: { message: string };
	meta?: unknown;
} {
	return (
		typeof err === 'object' &&
		err !== null &&
		'error' in err &&
		typeof err.error === 'object' &&
		'meta' in err &&
		typeof err.meta === 'object' &&
		'payload' in err &&
		typeof err.payload === 'string' &&
		'type' in err &&
		typeof err.type === 'string'
	);
}
