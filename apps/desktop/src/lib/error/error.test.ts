import { emitQueryError } from "$lib/error/error";
import { describe, expect, test, vi } from "vitest";
import type { PostHogWrapper } from "$lib/telemetry/posthog";

function fakePosthog() {
	const capture = vi.fn();
	return { posthog: { capture } as unknown as PostHogWrapper, capture };
}

/**
 * `query:error` is the "something is broken" telemetry stream. Errors the
 * classifier marks `silent` (offline network blips, known noise) are
 * expected states and must stay out of it, while real failures keep
 * flowing through with their severity attached.
 */
describe("emitQueryError", () => {
	test("rate limits separate list_reviews signatures independently", () => {
		const { posthog, capture } = fakePosthog();
		const context = { command: "list_reviews_signature_partition_test" };

		for (let index = 0; index < 5; index++) {
			emitQueryError(
				posthog,
				{
					name: "API error: (list_reviews)",
					message: "403 Forbidden: OAuth App access restrictions",
				},
				context,
			);
		}
		emitQueryError(
			posthog,
			{
				name: "API error: (list_reviews)",
				message: "Open pull request listing changed while paginating",
			},
			context,
		);

		expect(capture).toHaveBeenCalledTimes(6);
	});

	test("terminal states are captured once per session", () => {
		const { posthog, capture } = fakePosthog();
		const context = { command: "list_reviews_terminal_test", terminal: true };
		const error = {
			name: "API error: (list_reviews)",
			message: "403 Forbidden: OAuth App access restrictions",
		};

		for (let index = 0; index < 10; index++) {
			emitQueryError(posthog, error, context);
		}
		// A different terminal failure still gets its first capture.
		emitQueryError(
			posthog,
			{ ...error, message: "403 Forbidden: some other terminal state" },
			context,
		);

		expect(capture).toHaveBeenCalledTimes(2);
	});

	test("terminal states with a code dedupe on the code, not the message", () => {
		const { posthog, capture } = fakePosthog();
		const context = { command: "list_reviews_terminal_code_test", terminal: true };

		// The forge's wording can vary per org/repo; the code is the state.
		emitQueryError(
			posthog,
			{
				name: "API error: (list_reviews)",
				message: "403 Forbidden: org A has enabled restrictions",
				code: "GitHubOrgOAuthRestricted",
			},
			context,
		);
		emitQueryError(
			posthog,
			{
				name: "API error: (list_reviews)",
				message: "403 Forbidden: differently worded restriction",
				code: "GitHubOrgOAuthRestricted",
			},
			context,
		);

		expect(capture).toHaveBeenCalledTimes(1);
	});

	test("silent severity is not captured", () => {
		const { posthog, capture } = fakePosthog();
		emitQueryError(
			posthog,
			{ name: "API error", message: "Unable to connect to GitHub.", code: "NetworkError" },
			{ command: "list_ci_checks_silent_test", severity: "silent" },
		);
		expect(capture).not.toHaveBeenCalled();
	});

	test("error severity is captured with severity attached", () => {
		const { posthog, capture } = fakePosthog();
		emitQueryError(
			posthog,
			{ name: "API error", message: "boom", code: "Unknown" },
			{ command: "list_ci_checks_error_test", severity: "error" },
		);
		expect(capture).toHaveBeenCalledWith("query:error", {
			error_title: "API error",
			error_message: "boom",
			error_code: "Unknown",
			command: "list_ci_checks_error_test",
			actionName: undefined,
			severity: "error",
		});
	});

	test("errors without a severity still capture (callers outside the classifier)", () => {
		const { posthog, capture } = fakePosthog();
		emitQueryError(
			posthog,
			{ name: "API error", message: "boom" },
			{ command: "no_severity_test" },
		);
		expect(capture).toHaveBeenCalledOnce();
	});

	test("SilentError name is suppressed regardless of severity", () => {
		const { posthog, capture } = fakePosthog();
		emitQueryError(
			posthog,
			{ name: "SilentError", message: "handled elsewhere" },
			{ command: "silent_error_test", severity: "error" },
		);
		expect(capture).not.toHaveBeenCalled();
	});
});
