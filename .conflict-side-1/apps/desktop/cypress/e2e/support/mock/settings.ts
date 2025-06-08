import type { AppSettings, FeatureFlags, TelemetrySettings } from '$lib/config/appSettingsV2';

export const MOCK_TELEMETRY_SETINGS: TelemetrySettings = {
	appErrorReportingEnabled: false,
	appMetricsEnabled: false,
	appNonAnonMetricsEnabled: true
};

export const MOCK_FEATURE_FLAGS: FeatureFlags = {
	v3: true,
	ws3: false,
	actions: false
};

export const MOCK_APP_SETTINGS: AppSettings = {
	onboardingComplete: true,
	telemetry: MOCK_TELEMETRY_SETINGS,
	featureFlags: MOCK_FEATURE_FLAGS,
	fetch: {
		autoFetchIntervalMinutes: 15
	}
};
