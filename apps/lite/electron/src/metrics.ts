import { getAppSettings, getUserProfileLocal, updateTelemetryDistinctId } from "@gitbutler/but-sdk";
import type { UserProfile } from "@gitbutler/but-sdk";
import { PostHog } from "posthog-node";
import { getPosthogProjectToken, posthogHost } from "./telemetry.js";
import { randomUUID } from "node:crypto";
import { apiCommandFailureLimitConfig, createApiCommandSampler } from "./api-command-sampling.js";

/**
 * Product metrics and uncaught exceptions for the main process. Events go to the same PostHog
 * project as the desktop app, the web app, and the CLI; the `appName` and
 * `container` properties tell the surfaces apart. Conventions shared with
 * those senders: snake_case event names and camelCase properties. Product
 * metrics contain only counts, enums, and booleans — never message text,
 * paths, or anything else a user typed. Exceptions include error details.
 *
 * The distinct id is the one every surface shares through the settings file
 * (`telemetry.appDistinctId`): `user_<id>` once any surface has seen a login,
 * a random UUID otherwise. The CLI on the same machine reads whatever is
 * stored there.
 *
 * In PostHog, estimate successful command totals with
 * `sum(1 / coalesce(samplingRate, 1))`.
 * Failures are not randomly sampled; use `sum(coalesce(occurrenceCount, 1))`
 * because one retained event may represent suppressed repeats. Its `durationMs`
 * still describes only the retained call.
 */

const APP_NAME = "gitbutler-next";
const CONTAINER = "electron";
const SHUTDOWN_TIMEOUT_MS = 2000;
const ACTIVE_API_COMMAND_SHUTDOWN_GRACE_MS = SHUTDOWN_TIMEOUT_MS / 2;
const FAILURE_LIMIT_FLAG = "next-api-command-failure-limit";
const FAILURE_LIMIT_CONFIG_ID = "gitbutler-next-failure-limit";
const FAILURE_LIMIT_CONFIG_TIMEOUT_MS = 1_000;

let client: PostHog | null = null;
let distinctId = "";
let appVersion = "";
let metricsEnabled = false;
let errorsEnabled = false;
let appChannel: "dev" | "nightly" | "release";
let failureLimit = apiCommandFailureLimitConfig(undefined);
let apiCommandSampler = createApiCommandSampler({ failureLimit });
let shutdownPromise: Promise<void> | null = null;
const activeApiCommands = new Set<Promise<unknown>>();

const captureSuppressedFailures = (): void => {
	for (const failure of apiCommandSampler.drainSuppressedFailures())
		capture("api_command", { ...failure, failure: true, samplingRate: 1 });
};

const resetApiCommandSampler = (): void => {
	captureSuppressedFailures();
	apiCommandSampler = createApiCommandSampler({ failureLimit });
};

const setDistinctId = (nextDistinctId: string): void => {
	if (distinctId !== nextDistinctId) resetApiCommandSampler();
	distinctId = nextDistinctId;
};

const configureFailureLimit = async (metricsClient: PostHog): Promise<void> => {
	try {
		const payload = await metricsClient.getFeatureFlagPayload(
			FAILURE_LIMIT_FLAG,
			FAILURE_LIMIT_CONFIG_ID,
		);
		if (client !== metricsClient || shutdownPromise !== null) return;

		const nextFailureLimit = apiCommandFailureLimitConfig(payload);
		if (
			nextFailureLimit.bucketSize === failureLimit.bucketSize &&
			nextFailureLimit.refillIntervalMs === failureLimit.refillIntervalMs
		)
			return;

		failureLimit = nextFailureLimit;
		resetApiCommandSampler();
	} catch {
		// The built-in defaults are safe when remote configuration is unavailable.
	}
};

/**
 * Reads the shared app settings and starts the client when metrics or error
 * reporting is enabled. Dev builds send nothing. Never throws: telemetry
 * must not prevent startup.
 *
 * Await this before registering IPC handlers, so the first commands of the
 * session are captured and a launch via a login link cannot race the
 * identity below.
 */
export const initMetrics = async (
	version: string,
	environment: "development" | "production",
	channel: "dev" | "nightly" | "release",
): Promise<void> => {
	if (channel !== "nightly" && channel !== "release") return;

	try {
		const telemetry = (await getAppSettings()).telemetry;
		metricsEnabled = telemetry.appMetricsEnabled;
		errorsEnabled = telemetry.appErrorReportingEnabled;
		if (!metricsEnabled && !telemetry.appErrorReportingEnabled) return;

		appVersion = version;
		appChannel = channel;
		// The client exists from here on even if resolving the identity below
		// fails; a session then captures under the stored or fresh id.
		setDistinctId(telemetry.appDistinctId ?? randomUUID());
		client = new PostHog(getPosthogProjectToken(environment), {
			host: posthogHost,
			enableExceptionAutocapture: telemetry.appErrorReportingEnabled,
			before_send: (event) => {
				if (event?.event !== "$exception") return event;
				return {
					...event,
					distinctId,
					properties: {
						...event.properties,
						appName: APP_NAME,
						appVersion,
						appChannel,
						container: CONTAINER,
						process: "main",
						environment,
						$process_person_profile: distinctId.startsWith("user_"),
					},
				};
			},
			featureFlagsRequestTimeoutMs: FAILURE_LIMIT_CONFIG_TIMEOUT_MS,
		});

		const profile = await getUserProfileLocal();
		if (profile !== null) await metricsOnLogin(profile);
		else if (distinctId !== telemetry.appDistinctId) await updateTelemetryDistinctId(distinctId);
		// Not awaited: the window must not wait for PostHog. The built-in
		// default applies until the remote limit lands.
		if (metricsEnabled) void configureFailureLimit(client);
	} catch (error) {
		// oxlint-disable-next-line no-console
		console.error("Failed to initialize metrics", error);
	}
};

export const reportError = (error: unknown, context: string): void => {
	if (error instanceof Error && error.name === "AbortError") return;

	// oxlint-disable-next-line no-console
	console.error(context, error);
	if (errorsEnabled && shutdownPromise === null)
		client?.captureException(error, distinctId, { context });
};

const capture = (event: string, properties: Record<string, unknown>): void => {
	client?.capture({
		distinctId,
		event,
		properties: {
			...properties,
			appName: APP_NAME,
			appVersion,
			appChannel,
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
		const metricsClient = client;
		if (!metricsEnabled || metricsClient === null || shutdownPromise !== null)
			return handler(params);
		const start = performance.now();
		const record = (failure: boolean) => {
			if (client !== metricsClient) return;
			const decision = apiCommandSampler.sample(command, failure);
			if (decision === null) return;
			capture("api_command", {
				command,
				durationMs: Math.round(performance.now() - start),
				failure,
				...decision,
			});
		};
		let failure = false;
		const invocation = (async () => {
			try {
				return await handler(params);
			} catch (error) {
				failure = true;
				throw error;
			} finally {
				record(failure);
			}
		})();
		activeApiCommands.add(invocation);
		try {
			return await invocation;
		} finally {
			activeApiCommands.delete(invocation);
		}
	};

/**
 * Switches events to the account's identity after a login and shares it with
 * the other surfaces through the settings file. Never rejects, so callers may
 * fire-and-forget it. A sign-out deliberately keeps the identity — unlike the
 * desktop app, which resets it — since it is still the same person using us.
 */
export const metricsOnLogin = async (profile: UserProfile): Promise<void> => {
	if (client === null || shutdownPromise !== null) return;
	setDistinctId(`user_${profile.id}`);
	try {
		await updateTelemetryDistinctId(distinctId);
	} catch (error) {
		// oxlint-disable-next-line no-console
		console.error("Failed to persist the metrics distinct id", error);
	}
};

/**
 * Gives active API commands half the shutdown budget, then flushes queued
 * events with the time left. Returns null once there is nothing left to flush,
 * making the quit handler reentrant.
 */
const finishMetricsShutdown = async (flushing: PostHog): Promise<void> => {
	const deadline = performance.now() + SHUTDOWN_TIMEOUT_MS;
	if (activeApiCommands.size !== 0) {
		let timeout: ReturnType<typeof setTimeout> | undefined;
		await Promise.race([
			Promise.allSettled(activeApiCommands),
			new Promise<void>((resolve) => {
				timeout = setTimeout(resolve, ACTIVE_API_COMMAND_SHUTDOWN_GRACE_MS);
			}),
		]);
		if (timeout !== undefined) clearTimeout(timeout);
	}

	captureSuppressedFailures();
	client = null;
	await flushing.shutdown(Math.max(0, Math.ceil(deadline - performance.now())));
};

export const shutdownMetrics = (): Promise<void> | null => {
	if (shutdownPromise !== null) return shutdownPromise;
	if (client === null) return null;
	const flushing = client;
	shutdownPromise = finishMetricsShutdown(flushing)
		.catch(() => undefined)
		.finally(() => {
			shutdownPromise = null;
		});
	return shutdownPromise;
};
