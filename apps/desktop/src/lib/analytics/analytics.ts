import { PostHogWrapper } from '$lib/analytics/posthog';
import { initSentry } from '$lib/analytics/sentry';
import { AppSettings } from '$lib/config/appSettings';
import posthog from 'posthog-js';

export async function initAnalyticsIfEnabled(
	appSettings: AppSettings,
	postHog: PostHogWrapper,
	confirmedOverride?: boolean
) {
	if (import.meta.env.MODE === 'development' || import.meta.env.CI) return;

	const confirmed = confirmedOverride ?? (await appSettings.appAnalyticsConfirmed.onDisk());

	if (confirmed) {
		appSettings.appErrorReportingEnabled.onDisk().then((enabled) => {
			if (enabled) initSentry();
		});
		await appSettings.appMetricsEnabled.onDisk().then(async (enabled) => {
			if (enabled) {
				await postHog.init();
			}
		});
		appSettings.appNonAnonMetricsEnabled.onDisk().then((enabled) => {
			if (enabled) {
				posthog.capture('nonAnonMetricsEnabled');
			} else {
				posthog.capture('nonAnonMetricsDisabled');
			}
		});
	}
}
