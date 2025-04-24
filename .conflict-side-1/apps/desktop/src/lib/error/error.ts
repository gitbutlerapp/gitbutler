/**
 * Error type that has both a message and a status. These errors are primarily
 * thrown by AI services and the Octokit GitHub client.
 */
export interface HttpError {
	message: string;
	status: number;
}

/**
 * Error type that has both a message and a code. These errors can be thrown
 * by the back end code.
 */
export interface BackendError {
	message: string;
	code: string;
}

/**
 * Error type for unhandled Promise rejections.
 */
export interface UnhandledPromiseError {
	reason: Error;
	message: string;
}
