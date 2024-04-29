import { initPostHog } from '$lib/analytics/posthog';
import { initSentry } from '$lib/analytics/sentry';
import { appAnalyticsConfirmed } from '$lib/config/appSettings';
import { appMetricsEnabled, appErrorReportingEnabled } from '$lib/config/appSettings';

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
		}
	});
}
