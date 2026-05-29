import type { Code } from "@gitbutler/but-sdk";

export type ReduxError = {
	name?: string;
	message: string;
	code?: Code;
};

export function isReduxError(something: unknown): something is ReduxError {
	if (!something || typeof something !== "object") return false;
	const r = something as ReduxError;
	return (
		typeof r.message === "string" &&
		(r.name === undefined || typeof r.name === "string") &&
		(r.code === undefined || typeof r.code === "string")
	);
}

/**
 * Proper Error subclass for backend IPC failures.
 *
 * The Tauri (and web fallback) backend returns failures as plain
 * `{name?, message, code?}` objects. Throwing those raw means: no stack
 * trace, Sentry can't extract a fingerprint, and every variant collapses
 * into one generic "Object captured as promise rejection" bucket.
 *
 * Wrapping at the IPC boundary gives us a real Error instance with a
 * stack pointing at the actual caller, while still satisfying the
 * `ReduxError` duck-typed shape that downstream consumers (parser,
 * customHooks, backendQuery, baseBranchService) rely on.
 */
export class IpcError extends Error implements ReduxError {
	override readonly name: string;
	override readonly message: string;
	readonly code?: Code;

	constructor(raw: ReduxError, command: string) {
		super(raw.message);
		this.name = raw.name ?? `API error: (${command})`;
		// `Error.prototype.message` set via `super()` is a non-enumerable own
		// property, so `JSON.stringify(err)` would drop it — and any consumer
		// that captures `{error: ipcError}` to PostHog or similar would lose
		// the message. Redefine as enumerable to match the plain-object
		// `ReduxError` shape callers used to see.
		this.message = raw.message;
		this.code = raw.code;
	}
}
