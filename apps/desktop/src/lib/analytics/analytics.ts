import { initPostHog } from '$lib/analytics/posthog';
import { initSentry } from '$lib/analytics/sentry';
import { AppSettings } from '$lib/config/appSettings';
import posthog from 'posthog-js';

export function initAnalyticsIfEnabled(appSettings: AppSettings) {
	appSettings.appAnalyticsConfirmed.onDisk().then((confirmed) => {
		if (confirmed) {
			appSettings.appErrorReportingEnabled.onDisk().then((enabled) => {
				if (enabled) initSentry();
			});
			appSettings.appMetricsEnabled.onDisk().then((enabled) => {
				if (enabled) initPostHog();
			});
			appSettings.appNonAnonMetricsEnabled.onDisk().then((enabled) => {
				if (enabled) {
					posthog.capture('nonAnonMetricsEnabled');
				} else {
					posthog.capture('nonAnonMetricsDisabled');
				}
			});
		}
	});
}
