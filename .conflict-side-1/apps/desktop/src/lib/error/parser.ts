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
	message?: string;
	parsedError?: string;
}

export function isParsedError(something: unknown): something is ParsedError {
	return (
		typeof something === 'object' &&
		something !== null &&
		('message' in something || 'parsedError' in something)
	);
}

export function parseError(error: unknown): ParsedError {
	if (isTitledError(error)) {
		return parseError(error.error);
	}

	if (isParsedError(error)) {
		return error;
	}

	if (isStr(error)) {
		return { message: error };
	}

	if (error instanceof PromiseRejectionEvent && isTauriCommandError(error.reason)) {
		return { parsedError: error.reason.message };
	}

	if (isPromiseRejection(error)) {
		return {
			message: 'A promise had an unhandled exception.',
			parsedError: JSON.stringify(error.reason, null, 2)
		};
	}

	if (isTauriCommandError(error) && error.code && error.code in KNOWN_ERRORS) {
		return { message: KNOWN_ERRORS[error.code], parsedError: error.message };
	}

	if (isReduxActionError(error)) {
		return { parsedError: error.error + '\n\n' + error.payload };
	}

	if (isHttpError(error)) {
		// Silence GitHub octokit.js when disconnected. This should ideally be
		// prevented using `navigator.onLine` to avoid making requests when
		// working offline.
		if (error.status === 500 && error.message === 'Load failed') {
			return { message: undefined, parsedError: undefined };
		}
		return { parsedError: error.message };
	}

	if (isErrorlike(error)) {
		return { parsedError: error.message };
	}

	return { parsedError: JSON.stringify(error, null, 2) };
}

export type TitledError = {
	title: string;
	error: unknown;
};

export function isTitledError(error: unknown): error is TitledError {
	return (
		typeof error === 'object' &&
		error !== null &&
		'title' in error &&
		typeof error.title === 'string' &&
		error.title.length > 0 &&
		(error as any).error !== undefined
	);
}

export function createTitledError(title: string, error: unknown): TitledError {
	return { title, error };
}
