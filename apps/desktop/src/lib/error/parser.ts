import { KNOWN_ERRORS } from '$lib/error/knownErrors';
import {
	isBackendError,
	isHttpError,
	isPromiseRejection,
	isReduxActionError as isReduxActionError
} from '$lib/error/typeguards';
import { isErrorlike } from '@gitbutler/ui/utils/typeguards';

export interface ParsedError {
	message?: string;
	parsedError?: string;
}

export function parseError(error: unknown): ParsedError {
	if (isPromiseRejection(error)) {
		return { message: 'A promise had an unhandled exception.', parsedError: String(error.reason) };
	} else if (isBackendError(error) && error.code in KNOWN_ERRORS) {
		return { message: KNOWN_ERRORS[error.code], parsedError: error.message };
	} else if (isReduxActionError(error)) {
		return { parsedError: error.error + '\n\n' + error.payload };
	} else if (isHttpError(error)) {
		// Silence GitHub octokit.js when disconnected. This should ideally be
		// prevented using `navigator.onLine` to avoid making requests when
		// working offline.
		if (error.status === 500 && error.message === 'Load failed') {
			return { message: undefined, parsedError: undefined };
		}
		return { parsedError: error.message };
	} else if (isErrorlike(error)) {
		return { parsedError: error.message };
	} else {
		return { parsedError: String(error) };
	}
}
