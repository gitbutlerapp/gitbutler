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
		(('message' in something && typeof something.message === 'string') ||
			('parsedError' in something && typeof something.parsedError === 'string'))
	);
}

export function parseError(error: unknown): ParsedError {
	if (isNamedError(error)) {
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
			parsedError: String(error.reason)
		};
	}

	if (isTauriCommandError(error)) {
		if (error.code && error.code in KNOWN_ERRORS)
			return { message: KNOWN_ERRORS[error.code], parsedError: error.message };

		return { message: error.message };
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

export type NamedError = {
	name: string;
	error: unknown;
};

export function isNamedError(error: unknown): error is NamedError {
	return (
		typeof error === 'object' &&
		error !== null &&
		'name' in error &&
		typeof error.name === 'string' &&
		error.name.length > 0 &&
		(error as any).error !== undefined
	);
}

export function createNamedError(name: string, error: unknown): NamedError {
	return { name, error };
}
