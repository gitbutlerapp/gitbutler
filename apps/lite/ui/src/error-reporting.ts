import { type QueryClient, hashKey } from "@tanstack/react-query";
import posthog from "posthog-js/dist/module.no-external";
import "posthog-js/dist/exception-autocapture";
import {
	appSettingsQueryOptions,
	userProfileQueryOptions,
	versionQueryOptions,
} from "#ui/api/queries.ts";
import {
	getPosthogProjectToken,
	posthogHost,
	reportTelemetryInDev,
} from "../../electron/src/telemetry.ts";

export const initErrorReporting = async (queryClient: QueryClient): Promise<void> => {
	if (import.meta.env.DEV && !reportTelemetryInDev) return;

	try {
		const [settings, version, profile] = await Promise.all([
			queryClient.fetchQuery(appSettingsQueryOptions),
			queryClient.fetchQuery(versionQueryOptions),
			queryClient.fetchQuery(userProfileQueryOptions),
		]);

		if (!settings.telemetry.appErrorReportingEnabled) return;

		const environment = import.meta.env.DEV ? "development" : "production";
		posthog.init(getPosthogProjectToken(environment), {
			api_host: posthogHost,
			persistence: "memory",
			bootstrap: {
				distinctID: profile
					? `user_${profile.id}`
					: (settings.telemetry.appDistinctId ?? crypto.randomUUID()),
			},
			person_profiles: "never",
			autocapture: false,
			capture_pageview: false,
			capture_pageleave: false,
			capture_performance: false,
			disable_session_recording: true,
			disable_surveys: true,
			advanced_disable_flags: true,
			disable_external_dependency_loading: true,
			capture_exceptions: {
				capture_unhandled_errors: true,
				capture_unhandled_rejections: true,
				capture_console_errors: false,
			},
			before_send: (event) => {
				if (event?.event !== "$exception") return null;
				const exceptions = event.properties.$exception_list as Array<{ type: string }> | undefined;
				if (exceptions?.[0]?.type === "AbortError") return null;

				return event;
			},
		});

		const channel = process.env.CHANNEL;
		const properties = {
			appName: "gitbutler-next",
			appVersion: version,
			appChannel: channel === "nightly" || channel === "release" ? channel : "dev",
			container: "electron",
			process: "renderer",
			environment,
		};
		posthog.register(properties);

		queryClient.getQueryCache().subscribe((event) => {
			if (
				event.type !== "updated" ||
				event.query.queryHash !== hashKey(userProfileQueryOptions.queryKey)
			)
				return;

			const user = queryClient.getQueryData(userProfileQueryOptions.queryKey);
			if (user) posthog.register({ distinct_id: `user_${user.id}` });
		});
	} catch (error) {
		// oxlint-disable-next-line no-console
		console.error("Failed to initialize error reporting", error);
	}
};

export const reportError = (error: unknown, properties?: Record<string, unknown>): void => {
	if (error instanceof Error && error.name === "AbortError") return;

	// oxlint-disable-next-line no-console
	console.error(error);
	posthog.captureException(error, properties);
};
