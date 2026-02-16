import { PostHogWrapper } from "$lib/analytics/posthog";
import { initSentry } from "$lib/analytics/sentry";
import posthog from "posthog-js";
import type { Settings } from "@gitbutler/core/api";

export async function initAnalyticsIfEnabled(
	appSettings: Settings.AppSettings,
	postHog: PostHogWrapper,
	confirmedOverride?: boolean,
) {
	if (import.meta.env.MODE === "development" || import.meta.env.CI) return;

	const confirmed = confirmedOverride ?? appSettings.onboardingComplete;

	if (confirmed) {
		if (appSettings.telemetry.appErrorReportingEnabled) {
			initSentry();
		}
		if (appSettings.telemetry.appMetricsEnabled) {
			await postHog.init();
		}
		if (appSettings.telemetry.appNonAnonMetricsEnabled) {
			posthog.capture("nonAnonMetricsEnabled");
		} else {
			posthog.capture("nonAnonMetricsDisabled");
		}
	}
}
