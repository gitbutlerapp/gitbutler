import { isStr } from '@gitbutler/ui/utils/string';
import posthog from 'posthog-js';

/**
 * Error type that has both a message and a status. These errors are primarily
 * thrown by AI services and the Octokit GitHub client.
 */
export interface HttpError {
	message: string;
	status: number;
}

/**
 * Error type for unhandled Promise rejections.
 */
export interface UnhandledPromiseError {
	reason: Error;
	message: string;
}

/**
 * A subclass of `Error` that won't have a toast shown for it.
 *
 * This is useful for errors that are get handled elsewhere but should still
 * throw to stop execution.
 */
export class SilentError extends Error {
	constructor(message: string) {
		super(message);
		this.name = 'SilentError';
	}

	static from(error: Error): SilentError {
		const e = new SilentError(error.message);
		e.stack = error.stack;
		return e;
	}
}

const QUERY_ERROR_EVENT_NAME = 'query:error';
const DEFAULT_ERROR_NAME = 'QUERY:UnknownError';
const DEFAULT_ERROR_MESSAGE = 'QUERY:An unknown error occurred';

interface QueryError {
	name: string;
	message: string;
	code: string | undefined;
}

function isUnknownObject(error: unknown): error is Record<string, unknown> {
	return typeof error === 'object' && error !== null;
}

function getBestName(error: unknown): string {
	if (isStr(error)) return error;

	if (isUnknownObject(error) && 'name' in error && typeof error.name === 'string') {
		return error.name;
	}
	if (error instanceof Error) {
		return error.name;
	}

	return DEFAULT_ERROR_NAME;
}

function getBestMessage(error: unknown): string {
	if (isUnknownObject(error) && 'message' in error && typeof error.message === 'string') {
		return error.message;
	}

	if (error instanceof Error) {
		return error.message;
	}

	return DEFAULT_ERROR_MESSAGE;
}

function getBestCode(error: unknown): string | undefined {
	if (isUnknownObject(error) && 'code' in error && typeof error.code === 'string') {
		return error.code;
	}

	return undefined;
}

export function parseQueryError(error: unknown): QueryError {
	const name = getBestName(error);
	const message = getBestMessage(error);
	const code = getBestCode(error);

	return {
		name,
		message,
		code
	};
}

export function emitQueryError(error: unknown) {
	const { name, message, code } = parseQueryError(error);
	posthog.capture(QUERY_ERROR_EVENT_NAME, {
		erro_title: name,
		error_message: message,
		error_code: code
	});
}
