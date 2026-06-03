import { classify } from "$lib/error/errorClassification";
import { isNormalizedError, normalizedErrorToException } from "$lib/error/normalizedError";
import { shouldCaptureToast, showToast, type Toast } from "$lib/notifications/toasts";
import { captureException } from "@sentry/sveltekit";
import posthog from "posthog-js";

type ExtraAction = NonNullable<Toast["extraAction"]>;

export function showError(title: string, error: unknown, extraAction?: ExtraAction, id?: string) {
	const classified = classify(error, title);
	if (classified.severity === "silent") {
		return;
	}

	if (shouldCaptureToast()) {
		posthog.capture("toast:show_error", {
			error_test_id: id,
			error_title: classified.title,
			error_message: classified.message,
		});

		// Tauri rejections arrive as plain `{name, message, code}` objects
		// rather than `Error` instances. Sentry can't extract a stack from
		// those, so it buckets every variant under generic "Object captured
		// as promise rejection" groups. Wrap them in a proper Error so
		// Sentry groups by name + message.
		const forSentry =
			isNormalizedError(error) && !(error instanceof Error)
				? normalizedErrorToException(error)
				: error;
		captureException(forSentry, {
			mechanism: {
				// Surface the producer ("ipc" / "http" / "frontend") to
				// Sentry so triage can filter by where the error came from
				// without re-deriving it from the stack.
				type: isNormalizedError(error) ? (error.origin ?? "ui") : "ui",
				handled: true,
			},
		});
	}

	showToast({
		id,
		title: classified.title,
		message: classified.userMessage,
		error: classified.message,
		style: classified.severity === "warning" ? "warning" : "danger",
		extraAction: extraAction ?? classified.actionHint,
	});
}
