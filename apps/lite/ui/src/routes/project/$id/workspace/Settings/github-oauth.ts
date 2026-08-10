import { invalidateTags } from "#ui/api/tags.ts";
import type { QueryClient } from "@tanstack/react-query";
import { pollUntilSuccess } from "./poll.ts";

/** GitHub's device flow asks us not to poll faster than every five seconds. */
const pollIntervalMs = 5_000;
/** GitHub expires a device code after fifteen minutes; stop rather than poll a dead code. */
const pollTimeoutMs = 15 * 60 * 1000;

/**
 * Device-flow codes that will not change on a retry: the user refused, or the code died.
 * `but-github` reports them as `GitHub returned an error: <code> (<description>)`.
 */
const refusals = ["access_denied", "expired_token", "incorrect_client_credentials"];

const worthRetrying = (error: unknown): boolean => {
	const message = error instanceof Error ? error.message : String(error);
	return !refusals.some((code) => message.includes(code));
};

type DeviceFlow = {
	/** Shown to the user to type into GitHub. */
	userCode: string;
};

/**
 * Runs GitHub's device flow to completion.
 *
 * `onCode` fires as soon as there is a code to show, because the user needs it before
 * anything else can happen. Rejects with the signal's reason if it aborts.
 */
export const signInWithGithub = async ({
	client,
	onCode,
	signal,
}: {
	client: QueryClient;
	onCode: (flow: DeviceFlow) => void;
	signal: AbortSignal;
}): Promise<void> => {
	// Verification serializes snake_case, unlike its siblings in the same crate.
	const verification = await window.lite.initGithubDeviceOauth();
	onCode({ userCode: verification.user_code });
	await window.lite.openInWebBrowser("https://github.com/login/device");

	await pollUntilSuccess({
		// `authorization_pending` until the user finishes in the browser.
		attempt: () => window.lite.checkGithubAuthStatus(verification.device_code),
		intervalMs: pollIntervalMs,
		timeoutMs: pollTimeoutMs,
		signal,
		isRetryable: worthRetrying,
	});

	await invalidateTags(client, ["ForgeAccounts"]);
};
