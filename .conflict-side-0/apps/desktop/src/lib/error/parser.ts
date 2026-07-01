import { isNormalizedError, type NormalizedError } from "$lib/error/normalizedError";
import {
	isHttpError,
	isPromiseRejection,
	isReduxActionError as isReduxActionError,
} from "$lib/error/typeguards";
import { isStr } from "@gitbutler/ui/utils/string";
import { isErrorlike } from "@gitbutler/ui/utils/typeguards";

export function parseError(error: unknown): NormalizedError {
	if (isStr(error)) {
		return { origin: "unknown", message: error };
	}

	if (
		typeof PromiseRejectionEvent !== "undefined" &&
		error instanceof PromiseRejectionEvent &&
		isNormalizedError(error.reason)
	) {
		const { name, message, code } = error.reason;
		return { origin: "ipc", name, message, code };
	}

	if (isPromiseRejection(error)) {
		return {
			origin: "frontend",
			name: "A promise had an unhandled exception.",
			message: String(error.reason),
		};
	}

	if (isNormalizedError(error)) {
		const { name, message, code, origin } = error;
		// Keep the producer-set origin if present (e.g. IpcError sets
		// "ipc"); fall back to "ipc" since RTK's tauriBaseQuery is by
		// far the most common producer of this shape.
		return { origin: origin ?? "ipc", name, message, code };
	}

	if (isReduxActionError(error)) {
		return { origin: "ipc", message: error.error + "\n\n" + error.payload };
	}

	if (isHttpError(error)) {
		return { origin: "http", message: error.message };
	}

	if (isErrorlike(error)) {
		return { origin: "frontend", message: error.message };
	}

	return { origin: "unknown", message: JSON.stringify(error, null, 2) };
}
