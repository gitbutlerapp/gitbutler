import { SilentError } from "$lib/error/error";
import { parseError } from "$lib/error/parser";
import { isReduxError } from "$lib/error/reduxError";
import { showError } from "$lib/error/showError";
import { captureException } from "@sentry/sveltekit";

// Lazy-import logErrorToFile to avoid circular dependency with backend/.
let _logErrorToFile: ((error: string) => void) | undefined;

/**
 * Must be called at startup (e.g., from bootstrap) to wire up the backend
 * file logger.  Until then, errors are only logged to the console.
 */
export function setLogErrorToFile(fn: (error: string) => void) {
	_logErrorToFile = fn;
}

const E2E_MESSAGES_TO_IGNORE_DURING_E2E = [
	"Unable to autolaunch a dbus-daemon without a $DISPLAY for X11",
];
const E2E_MESSAGES_TO_IGNORE = [
	// We can safely ignore this error. This is caused by tauri falling back from the custom protocol to http protocol.
	"undefined is not an object (evaluating '[callbackId, data]')",
];

function shouldIgnoreError(error: unknown): boolean {
	const { message } = parseError(error);
	if (import.meta.env.VITE_E2E === "true") {
		return E2E_MESSAGES_TO_IGNORE_DURING_E2E.some((entry) => message.includes(entry));
	}

	return E2E_MESSAGES_TO_IGNORE.includes(message);
}

function loggableError(error: unknown): string {
	if (error instanceof Error) {
		return error.message;
	}

	if (typeof error === "string") {
		return error;
	}
	if (typeof error === "object" && error !== null) {
		if ("message" in error && typeof error.message === "string") {
			return error.message;
		}
		return JSON.stringify(error);
	}
	return String(error);
}

type LogErrorOptions = {
	/** If true, skip showing a toast notification (e.g. when a visual fallback is already shown). */
	skipToast?: boolean;
};

export function logError(error: unknown, options?: LogErrorOptions) {
	if (shouldIgnoreError(error)) {
		return;
	}

	try {
		// Unwrap promise rejections first so Sentry sees the underlying reason
		// rather than the event wrapper, and so SilentError detection works
		// against the actual thrown value.
		if (error instanceof PromiseRejectionEvent) {
			error = error.reason;
		}

		// `SilentError` indicates the caller already handled (or chose to
		// suppress) the error — skip both Sentry capture and the toast so
		// they don't double-up on noise or surface anything unexpected.
		const silent = error instanceof SilentError;

		if (!silent) {
			// Tauri rejections arrive as plain `{name, message, code}` objects
			// rather than `Error` instances. Sentry can't extract a stack from
			// those, so it buckets every variant under generic
			// "Object captured as promise rejection" groups. Wrap them in a
			// proper Error so Sentry groups by name + message.
			const forSentry =
				isReduxError(error) && !(error instanceof Error) ? reduxErrorToException(error) : error;
			captureException(forSentry, {
				mechanism: {
					type: "sveltekit",
					handled: false,
				},
			});
		}

		if (!options?.skipToast && !silent) {
			showError("Unhandled exception", error);
		}

		const logMessage = loggableError(error);
		_logErrorToFile?.(logMessage);

		console.error(error);
	} catch (err: unknown) {
		console.error("Error while trying to log error.", err);
	}
}

function reduxErrorToException(error: {
	name?: string;
	message: string;
	code?: string;
	fingerprint?: readonly string[];
}): Error {
	const err = new Error(error.message);
	// Prefer the backend-provided name (e.g. "API error: (push_stack)") over
	// the default "Error" so Sentry's title grouping matches the PostHog
	// taxonomy we already filter by.
	err.name = error.name || "Error";
	if (error.code) {
		(err as Error & { code?: string }).code = error.code;
	}
	if (error.fingerprint) {
		// `applyIpcFingerprint` (Sentry `beforeSend`) reads from this field
		// to apply the precomputed IpcError fingerprint to outgoing events.
		(err as Error & { fingerprint?: readonly string[] }).fingerprint = error.fingerprint;
	}
	return err;
}
