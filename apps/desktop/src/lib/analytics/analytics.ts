import { PostHogWrapper } from '$lib/analytics/posthog';
import { initSentry } from '$lib/analytics/sentry';
import { AppSettings } from '$lib/config/appSettings';
import { getName, getVersion } from '@tauri-apps/api/app';
import posthog from 'posthog-js';
import { PUBLIC_POSTHOG_API_KEY } from '$env/static/public';

export function initAnalyticsIfEnabled(appSettings: AppSettings, postHog: PostHogWrapper) {
	if (import.meta.env.MODE === 'development') return;

	appSettings.appAnalyticsConfirmed.onDisk().then((confirmed) => {
		if (confirmed) {
			appSettings.appErrorReportingEnabled.onDisk().then((enabled) => {
				if (enabled) initSentry();
			});
			appSettings.appMetricsEnabled.onDisk().then(async (enabled) => {
				if (enabled) {
					const [appName, appVersion] = await Promise.all([getName(), getVersion()]);
					postHog.init(appName, appVersion, PUBLIC_POSTHOG_API_KEY);
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
	});
}
