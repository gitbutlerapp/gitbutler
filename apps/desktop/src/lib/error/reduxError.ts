import type { Code } from "@gitbutler/but-sdk";
import type { ErrorEvent, EventHint } from "@sentry/sveltekit";

export type ReduxError = {
	name?: string;
	message: string;
	code?: Code;
	/**
	 * Optional Sentry fingerprint. Carried through the plain-object hop in
	 * `tauriBaseQuery` so `applyIpcFingerprint` can still find it after the
	 * original `IpcError` instance has been flattened by RTK Query. Without
	 * this, endpoint-driven errors lose their fingerprint en route to
	 * Sentry and fall back to default stack-based grouping.
	 */
	fingerprint?: readonly string[];
};

/**
 * Strip per-user / per-invocation identifiers from an error message so two
 * variants of the same root cause produce the same Sentry fingerprint.
 *
 * Tweak with care: this function is what makes distinct backend failures
 * inside one IPC command land in distinct Sentry issues. Loosening a rule
 * shatters previously-merged buckets; tightening one collapses unrelated
 * errors. Pair every change with a test in `reduxError.test.ts` that
 * pins the resulting fingerprint against a representative real message.
 */
function normalizeForFingerprint(text: string): string {
	const firstLine = text.split("\n", 1)[0] ?? text;
	return firstLine
		.replace(
			/\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b/g,
			"<uuid>",
		)
		.replace(/\b[0-9a-fA-F]{12,}\b/g, "<hex>")
		.replace(/:\d+:\d+\b/g, ":<line>:<col>")
		.replace(/\b\d{4,}\b/g, "<n>")
		.replace(/\/(?:Users|home|tmp|var|opt|run|private)\/[^\s"'\]]+/g, "<path>")
		.replace(/[A-Za-z]:\\[^\s"']+/g, "<path>")
		.replace(/refs\/(?:heads|remotes|tags)\/[^\s"')]+/g, "refs/<ref>")
		.replace(/(?<!:\/)\b[\w.-]+(?:\/[\w.-]+)+\b/g, "<path>")
		.replace(/"[A-Za-z0-9_][A-Za-z0-9_./-]{2,}"/g, '"<id>"')
		.replace(/'[A-Za-z0-9_][A-Za-z0-9_./-]{2,}'/g, "'<id>'")
		.replace(/\s+/g, " ")
		.trim();
}

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
	/**
	 * Stable Sentry fingerprint, applied by `beforeSend` in
	 * `analytics/sentry.ts`. Combines the command (so unrelated endpoints
	 * never collide) with the normalised first line of the message (so
	 * distinct backend failures inside one command — reflog write,
	 * worktree conflict, missing anchor — stay in separate issues instead
	 * of one mega-bucket).
	 */
	readonly fingerprint: readonly string[];

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
		this.fingerprint = ["ipc", command, normalizeForFingerprint(raw.message)];
	}
}

/**
 * Sentry `beforeSend` hook: copies a precomputed fingerprint onto the
 * outgoing event so backend failures inside one command split into
 * separate Sentry issues by root cause instead of stack-collapsing into
 * one mega-bucket.
 *
 * Reads from the exception's `fingerprint` property rather than checking
 * `instanceof IpcError`. This covers both:
 *   1. The direct path — an `IpcError` instance reaches Sentry whole
 *      (e.g. an unhandled rejection that didn't go through RTK Query).
 *   2. The RTK Query path — `tauriBaseQuery` flattens the `IpcError`
 *      into a plain `{name, message, code, fingerprint}` object and
 *      `reduxErrorToException` in `logError.ts` wraps it back into a
 *      generic `Error` with `fingerprint` copied across.
 *
 * Lives here (rather than next to `initSentry` in `analytics/sentry.ts`)
 * so the unit test can import it without pulling `@sentry/sveltekit`'s
 * runtime into jsdom — that runtime depends on SvelteKit aliases
 * (`$app/environment`) which vitest doesn't resolve inside `node_modules`.
 * Only the type aliases are imported here, which TypeScript erases.
 */
export function applyIpcFingerprint(event: ErrorEvent, hint?: EventHint): ErrorEvent {
	const exception = hint?.originalException;
	if (!exception || typeof exception !== "object") return event;
	const fp = (exception as { fingerprint?: unknown }).fingerprint;
	if (Array.isArray(fp) && fp.length > 0 && fp.every((s) => typeof s === "string")) {
		event.fingerprint = [...fp];
	}
	return event;
}
