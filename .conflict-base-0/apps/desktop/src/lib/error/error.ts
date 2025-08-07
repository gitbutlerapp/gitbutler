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
