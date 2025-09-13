import { Code } from '$lib/error/knownErrors';

export class UserError extends Error {
	code!: Code;
	cause: Error | undefined;

	constructor(message: string, code: Code, cause: Error | undefined) {
		super(message);
		this.cause = cause;
		this.code = code;
	}

	static fromError(error: any): UserError {
		const cause = error instanceof Error ? error : undefined;
		const code = error.code ?? Code.Unknown;
		const message = error.message ?? error;
		return new UserError(message, code, cause);
	}
}

export function getUserErrorCode(error: unknown): Code | undefined {
	if (error instanceof UserError) {
		return error.code;
	}
	const userError = UserError.fromError(error);
	return userError.code;
}
