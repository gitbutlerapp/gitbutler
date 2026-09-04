import GithubIntegration from "$components/settings/GithubIntegration.svelte";
import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
import { URL_SERVICE } from "$lib/backend/url";
import { GITHUB_USER_SERVICE } from "$lib/forge/github/githubUserService.svelte";
import { OnboardingEvent, POSTHOG_WRAPPER } from "$lib/telemetry/posthog";
import { chipToasts } from "@gitbutler/ui";
import { render, screen, waitFor } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, test, vi } from "vitest";
import type { GitHubUserService } from "$lib/forge/github/githubUserService.svelte";
import type { PostHogWrapper } from "$lib/telemetry/posthog";

// jsdom has no PointerEvent; ContextMenu.onMount does `target instanceof PointerEvent`.
(globalThis as any).PointerEvent ??= MouseEvent;

const DEVICE_CODE = "3584d274e7d7ec4aa8b2d9c2f6c0b1c7device";
const USER_CODE = "WDJB-MJHT";
const VERIFICATION_URL = "https://github.com/login/device";
/**
 * A `tauriBaseQuery` rejection for a terminal refusal. The backend sends a static
 * message for this code; the fixture carries raw detail anyway to prove the
 * component forwards nothing from the message.
 */
const rejection = {
	origin: "ipc",
	name: "API error: (check_github_auth_status)",
	message: `GitHub returned an error: access_denied (denied for device_code ${DEVICE_CODE}, user_code ${USER_CODE}; see ${VERIFICATION_URL})\n\nCaused by:\n    1: POST https://github.com/login/oauth/access_token params: {"deviceCode":"${DEVICE_CODE}"} Authorization: Bearer gho_fakeTokenValue at /home/kiril/.config/gitbutler`,
	code: "GitHubDeviceAccessDenied",
	fingerprint: ["ipc", "check_github_auth_status", "GitHub returned an error: access_denied"],
};
const rawFields = [
	"access_denied",
	DEVICE_CODE,
	USER_CODE,
	VERIFICATION_URL,
	"https://",
	"params",
	"Authorization:",
	"Bearer",
	"gho_",
	"/home/",
	"fingerprint",
	"origin",
	"API error",
	"\n",
];
const idle = { current: { isLoading: false } };

function renderIntegration(
	checkAuthStatus: () => Promise<unknown>,
	initDeviceOauth: () => Promise<unknown> = async () => ({
		user_code: USER_CODE,
		device_code: DEVICE_CODE,
	}),
) {
	const githubUserService = {
		initDeviceOauth: vi.fn(initDeviceOauth),
		checkAuthStatus: vi.fn(checkAuthStatus),
		deleteAllGitHubAccounts: () => [vi.fn(), idle],
		storeGitHubPat: [vi.fn(), idle],
		storeGithuibEnterprisePat: [vi.fn(), idle],
		accounts: () => ({ result: { data: [], status: "fulfilled" } }),
	} as unknown as GitHubUserService;
	const posthog = { capture: vi.fn(), captureOnboarding: vi.fn() } as unknown as PostHogWrapper;
	const context = new Map<any, any>([
		[GITHUB_USER_SERVICE._key, githubUserService],
		[POSTHOG_WRAPPER._key, posthog],
		[URL_SERVICE._key, { openExternalUrl: vi.fn(async () => {}) }],
		[CLIPBOARD_SERVICE._key, { write: vi.fn(async () => {}) }],
	]);
	const errorToast = vi.spyOn(chipToasts, "error").mockReturnValue("toast");
	const warningToast = vi.spyOn(chipToasts, "warning").mockReturnValue("toast");
	const successToast = vi.spyOn(chipToasts, "success").mockReturnValue("toast");
	const errorLog = vi.spyOn(console, "error").mockImplementation(() => {});
	render(GithubIntegration, { context });
	return { githubUserService, posthog, errorToast, warningToast, successToast, errorLog };
}

afterEach(() => vi.restoreAllMocks());

async function driveDeviceFlowToStatusCheck() {
	const user = userEvent.setup();
	await user.click(screen.getByRole("button", { name: /add account/i }));
	await user.click(await screen.findByText("Authorize GitHub Account"));
	await user.click(await screen.findByRole("button", { name: /copy to clipboard/i }));
	await user.click(await screen.findByRole("button", { name: /open github activation page/i }));
	// The status button is revealed 500ms after the activation page is opened.
	await user.click(
		await screen.findByRole("button", { name: /check the status/i }, { timeout: 3000 }),
	);
}

function expectFlowClosed() {
	expect(screen.queryByRole("button", { name: /check the status/i })).not.toBeInTheDocument();
	expect(screen.getByRole("button", { name: /add account/i })).toBeEnabled();
}

describe("GithubIntegration device OAuth failure", () => {
	test("shows and reports only the classifier's guidance for a terminal refusal", async () => {
		const { githubUserService, posthog, errorToast, warningToast, errorLog } = renderIntegration(
			async () => {
				throw rejection;
			},
		);

		await driveDeviceFlowToStatusCheck();

		await waitFor(() =>
			expect(githubUserService.checkAuthStatus).toHaveBeenCalledWith({ deviceCode: DEVICE_CODE }),
		);
		const payload = {
			name: "GitHub OAuth failed",
			message:
				"The authorization request was denied on GitHub. Start again and approve GitButler on the device activation page.",
			code: "GitHubDeviceAccessDenied",
		};
		await waitFor(() =>
			expect(posthog.captureOnboarding).toHaveBeenCalledWith(
				OnboardingEvent.GitHubOAuthFailed,
				payload,
			),
		);
		expect(posthog.captureOnboarding).toHaveBeenCalledTimes(2);
		// A denied request is a user state, so it toasts as a warning.
		expect(warningToast).toHaveBeenCalledTimes(1);
		expect(warningToast).toHaveBeenCalledWith(payload.message);
		expect(errorToast).not.toHaveBeenCalled();
		expect(errorLog).toHaveBeenCalledTimes(1);
		expect(errorLog.mock.calls.flat()).not.toContain(rejection);
		const serialized = JSON.stringify([
			vi.mocked(posthog.captureOnboarding).mock.calls,
			warningToast.mock.calls,
			errorLog.mock.calls,
		]);
		for (const raw of rawFields) expect(serialized).not.toContain(raw);
		expectFlowClosed();
	});

	test("reports an initialization refusal the same way and never opens the flow", async () => {
		const initRejection = {
			...rejection,
			name: "API error: (init_github_device_oauth)",
			message: rejection.message.replace("access_denied", "device_flow_disabled"),
			code: "GitHubDeviceFlowRejected",
		};
		const { posthog, errorToast, warningToast, errorLog } = renderIntegration(
			async () => ({ login: "octocat" }),
			async () => {
				throw initRejection;
			},
		);
		const user = userEvent.setup();

		await user.click(screen.getByRole("button", { name: /add account/i }));
		await user.click(await screen.findByText("Authorize GitHub Account"));

		const payload = {
			name: "GitHub OAuth failed",
			message:
				"GitHub rejected the device authorization request. Start again, or connect with a personal access token instead.",
			code: "GitHubDeviceFlowRejected",
		};
		await waitFor(() =>
			expect(posthog.captureOnboarding).toHaveBeenCalledWith(
				OnboardingEvent.GitHubOAuthFailed,
				payload,
			),
		);
		expect(vi.mocked(posthog.captureOnboarding).mock.calls).toEqual([
			[OnboardingEvent.GitHubInitiateOAuth],
			[OnboardingEvent.GitHubOAuthFailed, payload],
		]);
		expect(errorToast).toHaveBeenCalledTimes(1);
		expect(errorToast).toHaveBeenCalledWith(payload.message);
		expect(warningToast).not.toHaveBeenCalled();
		expect(errorLog).toHaveBeenCalledTimes(1);
		const serialized = JSON.stringify([
			vi.mocked(posthog.captureOnboarding).mock.calls,
			errorToast.mock.calls,
			errorLog.mock.calls,
		]);
		for (const raw of [...rawFields, "device_flow_disabled"]) expect(serialized).not.toContain(raw);
		expect(screen.queryByRole("button", { name: /copy to clipboard/i })).not.toBeInTheDocument();
		expect(screen.getByRole("button", { name: /add account/i })).toBeEnabled();
	});

	test("keeps the success path unchanged", async () => {
		const { posthog, errorToast, successToast } = renderIntegration(async () => ({
			login: "octocat",
		}));

		await driveDeviceFlowToStatusCheck();

		await waitFor(() => expectFlowClosed());
		expect(successToast).toHaveBeenCalledWith("GitHub authenticated");
		expect(errorToast).not.toHaveBeenCalled();
		expect(posthog.captureOnboarding).toHaveBeenCalledTimes(1);
		expect(posthog.captureOnboarding).toHaveBeenCalledWith(OnboardingEvent.GitHubInitiateOAuth);
	});
});
