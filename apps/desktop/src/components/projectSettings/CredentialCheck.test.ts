import CredentialCheck from "$components/projectSettings/CredentialCheck.svelte";
import { GIT_CONFIG_SERVICE } from "$lib/config/gitConfigService";
import { OnboardingEvent, POSTHOG_WRAPPER } from "$lib/telemetry/posthog";
import { render, screen, waitFor } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";
import type { GitConfigService } from "$lib/config/gitConfigService";
import type { NormalizedError } from "$lib/error/normalizedError";
import type { PostHogWrapper } from "$lib/telemetry/posthog";

const projectId = "private-project";
const remoteName = "private-remote";
const branchName = "private-branch";
const safeAuthError = {
	name: "Git credential check failed",
	message: "Authentication failed. Check that your git credentials are configured correctly.",
	code: "ProjectGitAuth",
};
const safeUnknownError = {
	name: "Git credential check failed",
	message: "Git credential check failed.",
	code: "Unknown",
};

function renderCredentialCheck({
	fetchError,
	pushError,
}: {
	fetchError?: NormalizedError;
	pushError?: NormalizedError;
} = {}) {
	const gitConfig = {
		checkGitFetch: vi.fn(async () => {
			if (fetchError) throw fetchError;
		}),
		checkGitPush: vi.fn(async () => {
			if (pushError) throw pushError;
		}),
	} as unknown as GitConfigService;
	const posthog = {
		capture: vi.fn(),
		captureOnboarding: vi.fn(),
	} as unknown as PostHogWrapper;
	const context = new Map<any, any>([
		[GIT_CONFIG_SERVICE._key, gitConfig],
		[POSTHOG_WRAPPER._key, posthog],
	]);

	render(CredentialCheck, {
		props: { projectId, remoteName, branchName, disabled: false },
		context,
	});

	return { gitConfig, posthog };
}

describe("CredentialCheck telemetry", () => {
	test("reports a normalized fetch failure with only its safe stage", async () => {
		const error: NormalizedError = {
			origin: "ipc",
			name: "Git fetch failed",
			message: "Credential helper failed for https://secret@example.com/private/repository.git",
			code: "ProjectGitAuth",
		};
		const { gitConfig, posthog } = renderCredentialCheck({ fetchError: error });
		const user = userEvent.setup();

		await user.click(screen.getByRole("button"));

		await waitFor(() =>
			expect(posthog.captureOnboarding).toHaveBeenCalledWith(
				OnboardingEvent.GitCheckCredentialsFailed,
				safeAuthError,
				{ stage: "fetch" },
			),
		);
		expect(posthog.captureOnboarding).toHaveBeenCalledTimes(1);
		expect(gitConfig.checkGitPush).not.toHaveBeenCalled();
	});

	test("reports a normalized push failure with only its safe stage", async () => {
		const error: NormalizedError = {
			origin: "ipc",
			name: "Git push failed",
			message: "Credential helper failed for /private/repository on private-branch",
			code: "Unknown",
		};
		const { posthog } = renderCredentialCheck({ pushError: error });
		const user = userEvent.setup();

		await user.click(screen.getByRole("button"));

		await waitFor(() =>
			expect(posthog.captureOnboarding).toHaveBeenCalledWith(
				OnboardingEvent.GitCheckCredentialsFailed,
				safeUnknownError,
				{ stage: "push" },
			),
		);
		expect(posthog.captureOnboarding).toHaveBeenCalledTimes(1);
	});

	test("does not report a failure when both checks pass", async () => {
		const { posthog } = renderCredentialCheck();
		const user = userEvent.setup();

		await user.click(screen.getByRole("button"));
		await screen.findByRole("button", { name: "Re-test credentials" });

		expect(posthog.capture).toHaveBeenCalledWith(OnboardingEvent.GitCheckCredentials);
		expect(posthog.capture).toHaveBeenCalledTimes(1);
		expect(posthog.captureOnboarding).not.toHaveBeenCalled();
	});
});
