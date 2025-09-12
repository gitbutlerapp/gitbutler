import type { AppSettings, FeatureFlags, TelemetrySettings } from '$lib/config/appSettingsV2';

export const MOCK_TELEMETRY_SETINGS: TelemetrySettings = {
	appErrorReportingEnabled: false,
	appMetricsEnabled: false,
	appNonAnonMetricsEnabled: true,
	appDistinctId: null
};

export const MOCK_FEATURE_FLAGS: FeatureFlags = {
	ws3: false,
	actions: false,
	butbot: false,
	rules: false,
	singleBranch: false
};

export const MOCK_APP_SETTINGS: AppSettings = {
	onboardingComplete: true,
	telemetry: MOCK_TELEMETRY_SETINGS,
	featureFlags: MOCK_FEATURE_FLAGS,
	fetch: {
		autoFetchIntervalMinutes: 15
	},
	claude: {
		executable: 'claude',
		notifyOnCompletion: false,
		notifyOnPermissionRequest: false,
		dangerouslyAllowAllPermissions: false,
		autoCommitAfterCompletion: true,
		useConfiguredModel: false
	}
};
