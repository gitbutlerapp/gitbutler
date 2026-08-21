import { getAppSettings, getUserProfileLocal, updateTelemetryDistinctId } from "@gitbutler/but-sdk";
import type { UserProfile } from "@gitbutler/but-sdk";
import { PostHog } from "posthog-node";
import { randomUUID } from "node:crypto";

/**
 * Product metrics for the main process. Events go to the same PostHog
 * project as the desktop app, the web app, and the CLI; the `appName` and
 * `container` properties tell the surfaces apart. Conventions shared with
 * those senders: snake_case event names, camelCase properties, and only
 * counts, enums, and booleans as values — never message text, paths, or
 * anything else a user typed.
 *
 * The distinct id is the one every surface shares through the settings file
 * (`telemetry.appDistinctId`): `user_<id>` once any surface has seen a login,
 * a random UUID otherwise. The CLI on the same machine reads whatever is
 * stored there.
 */

// The same publishable project key the desktop app and the web app use.
const POSTHOG_API_KEY = "phc_yJx46mXv6kA5KTuM2eEQ6IwNTgl5YW3feKV5gi7mfGG";
const POSTHOG_HOST = "https://eu.i.posthog.com";
const APP_NAME = "gitbutler-lite";
const CONTAINER = "electron";
const SHUTDOWN_TIMEOUT_MS = 2000;

let client: PostHog | null = null;
let distinctId = "";
let appVersion = "";

/**
 * Reads the shared app settings and starts the client. A no-op when metrics
 * are disabled. Never throws: the app must start even when metrics cannot.
 *
 * Await this before registering IPC handlers, so the first commands of the
 * session are captured and a launch via a login link cannot race the
 * identity below.
 */
export const initMetrics = async (version: string): Promise<void> => {
	try {
		const telemetry = (await getAppSettings()).telemetry;
		if (!telemetry.appMetricsEnabled) return;

		appVersion = version;
		// The client exists from here on even if resolving the identity below
		// fails; a session then captures under the stored or fresh id.
		distinctId = telemetry.appDistinctId ?? randomUUID();
		client = new PostHog(POSTHOG_API_KEY, { host: POSTHOG_HOST });

		const profile = await getUserProfileLocal();
		if (profile !== null) await metricsOnLogin(profile);
		else if (distinctId !== telemetry.appDistinctId) await updateTelemetryDistinctId(distinctId);
	} catch (error) {
		// oxlint-disable-next-line no-console
		console.error("Failed to initialize metrics", error);
	}
};

const capture = (event: string, properties: Record<string, unknown>): void => {
	client?.capture({
		distinctId,
		event,
		properties: {
			...properties,
			appName: APP_NAME,
			appVersion,
			container: CONTAINER,
			// Only events for a logged-in account may create a PostHog person
			// profile; anonymous installs stay person-less (and cheaper).
			$process_person_profile: distinctId.startsWith("user_"),
		},
	});
};

/**
 * Wraps an endpoint-table handler so every but-api call captures one
 * `api_command` event carrying the command name, its duration, and whether
 * it failed.
 */
export const withApiCommandCapture =
	(command: string, handler: (params: unknown) => unknown) =>
	async (params: unknown): Promise<unknown> => {
		if (client === null) return handler(params);
		const start = performance.now();
		const record = (failure: boolean) =>
			capture("api_command", {
				command,
				durationMs: Math.round(performance.now() - start),
				failure,
			});
		try {
			const result = await handler(params);
			record(false);
			return result;
		} catch (error) {
			record(true);
			throw error;
		}
	};

/**
 * Switches events to the account's identity after a login and shares it with
 * the other surfaces through the settings file. Never rejects, so callers may
 * fire-and-forget it. A sign-out deliberately keeps the identity — unlike the
 * desktop app, which resets it — since it is still the same person using us.
 */
export const metricsOnLogin = async (profile: UserProfile): Promise<void> => {
	if (client === null) return;
	distinctId = `user_${profile.id}`;
	try {
		await updateTelemetryDistinctId(distinctId);
	} catch (error) {
		// oxlint-disable-next-line no-console
		console.error("Failed to persist the metrics distinct id", error);
	}
};

/**
 * Flushes queued events, since the client only holds them in memory. Returns
 * null when there is nothing to flush, so quitting can proceed immediately —
 * and does so on the second call, making the quit handler reentrant.
 */
export const shutdownMetrics = (): Promise<void> | null => {
	if (client === null) return null;
	const flushing = client;
	client = null;
	return flushing.shutdown(SHUTDOWN_TIMEOUT_MS).catch(() => undefined);
};
