/**
 * Fetch and push are checked in turn against the target's remote, because a push
 * failure after a clean fetch says something different from both failing.
 */
type CheckName = "Fetch" | "Push";

type CredentialCheck = {
	name: CheckName;
	/** Absent while the check is still running. */
	error?: string;
};

export type CredentialCheckState =
	| { _tag: "Idle" }
	| { _tag: "Running"; checks: Array<CredentialCheck> }
	| { _tag: "Done"; checks: Array<CredentialCheck> };

export const failed = (state: CredentialCheckState): boolean =>
	state._tag !== "Idle" && state.checks.some((check) => check.error !== undefined);

/** First line only: these come back as git's full stderr, which is many lines of it. */
export const firstLine = (error: unknown): string =>
	(error instanceof Error ? error.message : String(error)).split("\n")[0] ?? "Failed";
