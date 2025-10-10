import { getSwallowGitHubOrgAuthErrors } from '$lib/config/config';
import { KNOWN_ERRORS } from '$lib/error/knownErrors';
import {
	isHttpError,
	isPromiseRejection,
	isReduxActionError as isReduxActionError
} from '$lib/error/typeguards';
import { isReduxError } from '$lib/state/reduxError';
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

/**
 * It appears that Vite sporadically experiences some bundling error where
 * a resource that no longer exists is requested. The fastest way to resolve
 * such an error is to disable the cache from the network tab and reload the
 * page once. It would be great if we could root cause and fix this problem.
 */
export function isBundlingError(message: string): boolean {
	return message.startsWith("undefined is not an object (evaluating 'first_child_getter.call')");
}

export function parseError(error: unknown): ParsedError {
	if (isStr(error)) {
		return { message: error };
	}

	if (error instanceof PromiseRejectionEvent && isReduxError(error.reason)) {
		const { name, message, code } = error.reason;
		return { name, message, code };
	}

	if (isPromiseRejection(error)) {
		return {
			name: 'A promise had an unhandled exception.',
			message: String(error.reason)
		};
	}

	if (isReduxError(error)) {
		const { name, message, code } = error;
		const description = code ? KNOWN_ERRORS[code] : undefined;
		return { name, message, code, description };
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

const GH_ORG_AUTH_ERROR = 'GitHub Organizations OAuth Error';

export function isGitHubOrgAuthError(title: string): boolean {
	return title === GH_ORG_AUTH_ERROR;
}

export function shouldOfferToIgnoreError(title: string): boolean {
	return isGitHubOrgAuthError(title);
}

export function shouldIgnoreThistError(title: string): boolean {
	if (isGitHubOrgAuthError(title)) {
		return getSwallowGitHubOrgAuthErrors();
	}
	return false;
}

const CommonErrorMessageStart: Record<string, string> = {
	'Although you appear to have the correct authorization credentials,': GH_ORG_AUTH_ERROR
};
/**
 * Returns an unified title for common error messages.
 *
 * This is used mainly to group errors unders a readable title and be able to graph them into the same group.
 */
export function getTitleFromCommonErrorMessage(errorMessage: string): string | undefined {
	for (const [start, title] of Object.entries(CommonErrorMessageStart)) {
		if (errorMessage.startsWith(start)) {
			return title;
		}
	}
	return undefined;
}
