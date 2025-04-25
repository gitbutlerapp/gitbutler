import { isTauriCommandError } from '$lib/backend/ipc';
import { KNOWN_ERRORS } from '$lib/error/knownErrors';
import {
	isHttpError,
	isPromiseRejection,
	isReduxActionError as isReduxActionError
} from '$lib/error/typeguards';
import { isStr } from '@gitbutler/ui/utils/string';
import { isErrorlike } from '@gitbutler/ui/utils/typeguards';

export interface ParsedError {
	message: string;
	name?: string;
	code?: string;
	ignored?: boolean;
	description?: string;
}

export function isParsedError(something: unknown): something is ParsedError {
	return (
		typeof something === 'object' &&
		something !== null &&
		'message' in something &&
		typeof something.message === 'string'
	);
}

export function parseError(error: unknown): ParsedError {
	if (isStr(error)) {
		return { message: error };
	}

	if (error instanceof PromiseRejectionEvent && isTauriCommandError(error.reason)) {
		return { message: error.reason.message };
	}

	if (isPromiseRejection(error)) {
		return {
			name: 'A promise had an unhandled exception.',
			message: String(error.reason)
		};
	}

	if (isTauriCommandError(error)) {
		if (error.code && error.code in KNOWN_ERRORS)
			return { description: KNOWN_ERRORS[error.code], message: error.message };

		return { message: error.message, code: error.code };
	}

	if (isReduxActionError(error)) {
		return { message: error.error + '\n\n' + error.payload };
	}

	if (isHttpError(error)) {
		// Silence GitHub octokit.js when disconnected. This should ideally be
		// prevented using `navigator.onLine` to avoid making requests when
		// working offline.
		if (error.status === 500 && error.message === 'Load failed') {
			return { message: error.message, ignored: true };
		}
		return { message: error.message };
	}

	if (isErrorlike(error)) {
		return { message: error.message };
	}

	return { message: JSON.stringify(error, null, 2) };
}
