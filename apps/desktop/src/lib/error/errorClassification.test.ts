import { SilentError } from "$lib/error/error";
import { classify } from "$lib/error/errorClassification";
import { IpcError } from "$lib/error/normalizedError";
import { describe, expect, test } from "vitest";

/**
 * The classifier owns "how should this error show up to the user?"
 * for the whole desktop app. Pin the contract here: adding a new code
 * to `CLASSIFICATIONS` should not change the default-path behaviour
 * for everything else, and the silent rules in particular must never
 * leak into the toast pipeline.
 */
describe("classify", () => {
	describe("severity: silent", () => {
		test("SilentError suppresses everything", () => {
			const result = classify(new SilentError("ignored"), "Should not show");
			expect(result.severity).toBe("silent");
		});

		test("unrecoverable Vite bundling error is suppressed", () => {
			const error = new Error("undefined is not an object (evaluating 'first_child_getter.call')");
			expect(classify(error).severity).toBe("silent");
		});

		test("octokit 'Load failed' HTTP 500 (parser-flagged ignored) is suppressed", () => {
			// `code` must be a non-string so the error fails
			// `isNormalizedError`'s duck-type and falls through to the
			// parser's HTTP branch, where origin='http' lets classify
			// recognise the "Load failed" + 500 pattern as silent.
			const error = { message: "Load failed", status: 500, code: 500 };
			expect(classify(error).severity).toBe("silent");
		});

		test("DefaultTargetNotFound is silent — router redirects to setup", () => {
			const error = new IpcError(
				{ message: "no default target", code: "DefaultTargetNotFound" },
				"fetchFromRemotes",
			);
			expect(classify(error).severity).toBe("silent");
		});
	});

	describe("message-pattern classifications", () => {
		test("Unknown + 'cargo build -p gitbutler-git' adds the build-binary hint", () => {
			const error = new IpcError(
				{
					message: "failed to run cargo build -p gitbutler-git",
					code: "Unknown",
				},
				"fetchFromRemotes",
			);
			const result = classify(error);
			expect(result.severity).toBe("error");
			expect(result.userMessage).toContain("gitbutler-git");
			expect(result.userMessage).toContain("cargo build");
		});
	});

	describe("severity: warning", () => {
		test("PreconditionFailed code downgrades to warning", () => {
			const error = new IpcError(
				{ message: "branch isn't applied", code: "PreconditionFailed" },
				"some_command",
			);
			const result = classify(error);
			expect(result.severity).toBe("warning");
			expect(result.code).toBe("PreconditionFailed");
		});

		test("ProjectGitAuth surfaces an auth-credentials hint", () => {
			const error = new IpcError(
				{ message: "no credentials available", code: "ProjectGitAuth" },
				"fetchFromRemotes",
			);
			const result = classify(error);
			expect(result.severity).toBe("warning");
			expect(result.userMessage).toContain("credentials");
		});
	});

	describe("severity: error with userMessage", () => {
		test("CommitSigningFailed exposes the long-form remediation copy", () => {
			const error = new IpcError(
				{ message: "gpg refused", code: "CommitSigningFailed" },
				"commit_create",
			);
			const result = classify(error);
			expect(result.severity).toBe("error");
			expect(result.userMessage).toContain("signing failed");
			expect(result.userMessage).toContain("documentation");
		});

		test("GitHubTokenExpired tells the user to log out and back in", () => {
			const error = new IpcError(
				{ message: "token rejected", code: "GitHubTokenExpired" },
				"github_request",
			);
			const result = classify(error);
			expect(result.userMessage).toContain("expired");
			expect(result.userMessage).toContain("log out");
		});
	});

	describe("default severity", () => {
		test("an unrecognised code falls through to severity: error", () => {
			const error = new IpcError({ message: "boom" }, "some_command");
			const result = classify(error);
			expect(result.severity).toBe("error");
			expect(result.userMessage).toBeUndefined();
			expect(result.actionHint).toBeUndefined();
		});
	});

	describe("title resolution", () => {
		test("uses the error's own name when present (IPC commands)", () => {
			const error = new IpcError({ message: "boom" }, "push_stack");
			expect(classify(error, "Caller title").title).toBe("API error: (push_stack)");
		});

		test("falls back to the caller title when the error has no name", () => {
			// String inputs take the parser's `isStr` branch which leaves
			// `name` unset, so the title falls through to the caller arg.
			expect(classify("raw", "Failed to delete project").title).toBe("Failed to delete project");
		});

		test("falls back to the raw message when neither name nor caller title is set", () => {
			expect(classify("raw").title).toBe("raw");
		});

		test("rewrites the GitHub-org-auth message prefix to a stable title", () => {
			expect(
				classify(
					"Although you appear to have the correct authorization credentials, the org has SSO enforced.",
				).title,
			).toBe("GitHub Organizations OAuth Error");
		});
	});
});
