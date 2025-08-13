import { PostHogWrapper } from '$lib/analytics/posthog';
import { initSentry } from '$lib/analytics/sentry';
import { AppSettings } from '$lib/config/appSettings';
import { getName, getVersion } from '@tauri-apps/api/app';
import posthog from 'posthog-js';

export function initAnalyticsIfEnabled(appSettings: AppSettings, postHog: PostHogWrapper) {
	if (import.meta.env.MODE === 'development') return;

	appSettings.appAnalyticsConfirmed.onDisk().then((confirmed) => {
		if (confirmed) {
			appSettings.appErrorReportingEnabled.onDisk().then((enabled) => {
				if (enabled) initSentry();
			});
			appSettings.appMetricsEnabled.onDisk().then(async (enabled) => {
				if (enabled) {
					if (import.meta.env.VITE_BUILD_TARGET === 'web') {
						postHog.init('gitbutler-web', '0.0.0');
					} else {
						const [appName, appVersion] = await Promise.all([getName(), getVersion()]);
						postHog.init(appName, appVersion);
					}
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
