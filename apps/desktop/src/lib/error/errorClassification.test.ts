import { persistSwallowGitHubOrgAuthErrors } from "$lib/config/config";
import { SilentError } from "$lib/error/error";
import { classify, classifyGitHubDeviceOAuthFailure } from "$lib/error/errorClassification";
import { IpcError } from "$lib/error/normalizedError";
import { describe, expect, test } from "vitest";
import type { Code } from "@gitbutler/but-sdk";

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

		test("NetworkError is silent — offline forge polls shouldn't toast", () => {
			const error = new IpcError(
				{ message: "Unable to connect to GitHub.", code: "NetworkError" },
				"list_reviews",
			);
			expect(classify(error).severity).toBe("silent");
		});

		test("opted-out org OAuth errors stay terminal so pollers still stop", () => {
			persistSwallowGitHubOrgAuthErrors(true);
			try {
				const error = new IpcError(
					{
						message: "the organization has enabled OAuth App access restrictions",
						code: "GitHubOrgOAuthRestricted",
					},
					"list_ci_checks",
				);
				const result = classify(error);
				expect(result.severity).toBe("silent");
				expect(result.terminal).toBe(true);
			} finally {
				persistSwallowGitHubOrgAuthErrors(false);
			}
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

		test("ForgeUnrecognized is a terminal warning with guidance, not silence", () => {
			// `list_reviews` tags a target remote that maps to no supported forge.
			// Pollers must stop on it, while an explicit Sync still gets told why.
			const error = new IpcError(
				{
					message: "No forge could be determined for this repository branch",
					code: "ForgeUnrecognized",
				},
				"list_reviews",
			);
			const result = classify(error);
			expect(result.severity).toBe("warning");
			expect(result.terminal).toBe(true);
			expect(result.message).toBe("No forge could be determined for this repository branch");
			expect(result.userMessage).toContain("target branch");
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

		test("GitHubInsufficientPermissions is terminal with permission guidance", () => {
			const error = new IpcError(
				{
					message: "Your GitHub credentials don't have permission to read this.",
					code: "GitHubInsufficientPermissions",
				},
				"list_ci_checks",
			);
			const result = classify(error);
			expect(result.severity).toBe("error");
			// Terminal: telemetry captures once per session and pollers stop.
			expect(result.terminal).toBe(true);
			expect(result.userMessage).toContain("permission");
		});

		test("GitHubOrgSamlRestricted is terminal with credential-neutral SSO guidance", () => {
			const error = new IpcError(
				{
					message: "This GitHub organization requires SAML SSO authorization.",
					code: "GitHubOrgSamlRestricted",
				},
				"list_reviews",
			);
			const result = classify(error);

			expect(result.code).toBe("GitHubOrgSamlRestricted");
			expect(result.severity).toBe("error");
			expect(result.terminal).toBe(true);
			expect(result.title).toBe("GitHub SAML SSO Authorization Required");
			expect(result.userMessage).toContain("OAuth app");
			expect(result.userMessage).toContain("personal access token");
			expect(result.userMessage).toContain("then try again");
		});
	});

	describe("GitHub device-flow codes", () => {
		test.each<[Code, string, RegExp]>([
			["GitHubDeviceCodeExpired", "warning", /device code has expired/],
			["GitHubDeviceAccessDenied", "warning", /was denied on GitHub/],
			["GitHubDeviceFlowRejected", "error", /rejected the device authorization/],
		])("%s carries %s severity and recovery guidance", (code, severity, guidance) => {
			const error = new IpcError({ message: "static", code }, "check_github_auth_status");
			const result = classify(error);
			expect(result.code).toBe(code);
			expect(result.severity).toBe(severity);
			expect(result.userMessage).toMatch(guidance);
		});

		test("pending statuses and arbitrary codes get no guidance", () => {
			const pending = new IpcError(
				{
					message:
						"GitHub returned an error: authorization_pending (The authorization request is still pending.)",
					code: "Unknown",
				},
				"check_github_auth_status",
			);
			expect(classify(pending).userMessage).toBeUndefined();
			const arbitrary = { message: "failure", code: "constructor" };
			expect(classify(arbitrary).userMessage).toBeUndefined();
		});
	});

	describe("classifyGitHubDeviceOAuthFailure", () => {
		const generic = { message: "GitHub authentication failed", severity: "error" };
		const raw =
			"GitHub returned an error: access_denied (device_code 3584d274 https://github.com/login/device)";

		test.each<[Code, string, RegExp]>([
			["GitHubDeviceCodeExpired", "warning", /device code has expired/],
			["GitHubDeviceAccessDenied", "warning", /was denied on GitHub/],
			["GitHubDeviceFlowRejected", "error", /rejected the device authorization/],
		])("%s yields its static guidance, code, and %s severity", (code, severity, guidance) => {
			const result = classifyGitHubDeviceOAuthFailure({ message: raw, code });
			expect(result).toEqual({ message: expect.stringMatching(guidance), code, severity });
			expect(result.message).not.toContain("3584d274");
		});

		test.each([
			[
				"a pending status",
				{ message: raw.replace("access_denied", "authorization_pending"), code: "Unknown" },
			],
			["a code that is only an inherited key", { message: raw, code: "constructor" }],
			["a code that is a prototype key", { message: raw, code: "__proto__" }],
			["a secret-rich code string", { message: raw, code: "gho_secret" }],
			["a thrown string", raw],
			["undefined", undefined],
			["a bigint", 1n],
			[
				"a cyclic object",
				(() => {
					const c: Record<string, unknown> = {};
					c.self = c;
					return c;
				})(),
			],
		])("%s falls back to the generic label without a code", (_, error) => {
			expect(classifyGitHubDeviceOAuthFailure(error)).toEqual(generic);
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
		test("recognises a GitHub org OAuth restriction from a real list_reviews IPC error", () => {
			// The backend tags the org-restriction 403 with a dedicated code
			// (`classify_forge_error` in `but-github`); the classification's
			// title override must beat the generic `API error: (<command>)`
			// name so the opt-out action is reachable.
			const error = new IpcError(
				{
					message:
						'Failed to list open pull requests\n\nCaused by:\n    1: 403 Forbidden: {"message":"Although you appear to have the correct authorization credentials, the organization has enabled OAuth App access restrictions."}\n    2: HTTP 403',
					code: "GitHubOrgOAuthRestricted",
				},
				"list_reviews",
			);

			const result = classify(error);

			expect(result.title).toBe("GitHub Organizations OAuth Error");
			expect(result.actionHint?.label).toBe("Don't show this again");
		});

		test("uses the error's own name when present (IPC commands)", () => {
			const error = new IpcError({ message: "boom" }, "workspace_branch_and_ancestors_push");
			expect(classify(error, "Caller title").title).toBe(
				"API error: (workspace_branch_and_ancestors_push)",
			);
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
			// The octokit path carries no backend code, so the message
			// pattern must yield the same classification as the code entry.
			const result = classify(
				"Although you appear to have the correct authorization credentials, the org has SSO enforced.",
			);
			expect(result.title).toBe("GitHub Organizations OAuth Error");
			expect(result.actionHint?.label).toBe("Don't show this again");
		});
	});
});
