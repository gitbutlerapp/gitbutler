import type { Code } from "@gitbutler/but-sdk";

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
		// `error` is `any`, so `error.code` could be anything at runtime
		// (or missing entirely). Anything other than a non-empty string
		// gets bucketed as "Unknown"; a known string is trusted to be a
		// `Code` variant — the wire format is the source of truth and a
		// future backend may legitimately emit codes this build hasn't
		// seen yet.
		const rawCode: unknown = error?.code;
		const code: Code =
			typeof rawCode === "string" && rawCode.length > 0 ? (rawCode as Code) : "Unknown";
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
