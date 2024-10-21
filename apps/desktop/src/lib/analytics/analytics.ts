import { initPostHog } from '$lib/analytics/posthog';
import { initSentry } from '$lib/analytics/sentry';
import { appAnalyticsConfirmed } from '$lib/config/appSettings';
import {
	appMetricsEnabled,
	appErrorReportingEnabled,
	appNonAnonMetricsEnabled
} from '$lib/config/appSettings';
import posthog from 'posthog-js';

export function initAnalyticsIfEnabled() {
	const analyticsConfirmed = appAnalyticsConfirmed();
	analyticsConfirmed.onDisk().then((confirmed) => {
		if (confirmed) {
			appErrorReportingEnabled()
				.onDisk()
				.then((enabled) => {
					if (enabled) initSentry();
				});
			appMetricsEnabled()
				.onDisk()
				.then((enabled) => {
					if (enabled) initPostHog();
				});
			appNonAnonMetricsEnabled()
				.onDisk()
				.then((enabled) => {
					if (enabled) {
						posthog.capture('nonAnonMetricsEnabled');
					} else {
						posthog.capture('nonAnonMetricsDisabled');
					}
				});
		}
	});
}
