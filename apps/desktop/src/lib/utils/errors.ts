const FAILED_TO_OPEN_REPO_INEXISTENT_PATTERN =
	/^Could not open repository at '(.+)' as it does not exist$/;

export enum KnownErrorType {
	FailedToOpenRepoInexistent = 'FailedToOpenRepoInexistent'
}

interface BaseKnownError {
	type: KnownErrorType;
}

export interface FailedToOpenRepoInexistentError extends BaseKnownError {
	type: KnownErrorType.FailedToOpenRepoInexistent;
	path: string;
}

export type KnownError = FailedToOpenRepoInexistentError;

export function getErrorMessage(something: unknown): string | null {
	if (something instanceof Error) return something.message;
	if (typeof something === 'string') return something;
	return null;
}

export function getErrorType(something: unknown): KnownErrorType | null {
	const message = getErrorMessage(something);
	if (!message) return null;
	if (FAILED_TO_OPEN_REPO_INEXISTENT_PATTERN.test(message)) {
		return KnownErrorType.FailedToOpenRepoInexistent;
	}
	return null;
}

export function getKnownError(something: unknown): KnownError | null {
	const type = getErrorType(something);
	if (!type) return null;
	const message = getErrorMessage(something);
	if (!message) return null;

	switch (type) {
		case KnownErrorType.FailedToOpenRepoInexistent: {
			const match = message.match(FAILED_TO_OPEN_REPO_INEXISTENT_PATTERN);
			if (!match) return null;
			return {
				type,
				path: match[1] ?? ''
			};
		}
	}
}
