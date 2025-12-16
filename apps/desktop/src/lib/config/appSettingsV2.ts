import { InjectionToken } from '@gitbutler/core/context';
import { writable } from 'svelte/store';
import type { IBackend } from '$lib/backend';

export const SETTINGS_SERVICE = new InjectionToken<SettingsService>('SettingsService');

export class SettingsService {
	readonly appSettings = writable<AppSettings | undefined>(undefined, () => {
		this.refresh();
		const unsubscribe = this.listen(async (settings) => await this.handlePayload(settings));
		return () => {
			unsubscribe();
		};
	});

	readonly subscribe = this.appSettings.subscribe;

	constructor(private backend: IBackend) {}

	private async handlePayload(settings: AppSettings) {
		this.appSettings.set(settings);
	}

	async refresh() {
		const response = await this.backend.invoke<AppSettings>('get_app_settings');
		const settings = response;
		this.handlePayload(settings);
	}

	private listen(callback: (settings: AppSettings) => void) {
		return this.backend.listen<AppSettings>(`settings://update`, (event) =>
			callback(event.payload)
		);
	}

	async updateOnboardingComplete(update: boolean) {
		await this.backend.invoke('update_onboarding_complete', { update });
	}

	async updateTelemetry(update: Partial<TelemetrySettings>) {
		await this.backend.invoke('update_telemetry', { update });
	}

	async updateTelemetryDistinctId(appDistinctId: string | null) {
		await this.backend.invoke('update_telemetry_distinct_id', { appDistinctId });
	}

	async updateFeatureFlags(update: Partial<FeatureFlags>) {
		await this.backend.invoke('update_feature_flags', { update });
	}

	async updateClaude(update: Partial<Claude>) {
		await this.backend.invoke('update_claude', { update });
	}

	async updateReviews(update: Partial<Reviews>) {
		await this.backend.invoke('update_reviews', { update });
	}

	async updateFetch(update: Partial<Fetch>) {
		await this.backend.invoke('update_fetch', { update });
	}

	async updateUi(update: Partial<UiSettings>) {
		await this.backend.invoke('update_ui', { update });
	}

	/**
	 * For all projects this call deletes the following:
	 * - project meta data directory
	 * - project data directory
	 */
	async deleteAllData() {
		await this.backend.invoke<void>('delete_all_data');
	}
}

export type AppSettings = {
	/** Whether the user has passed the onboarding flow. */
	onboardingComplete: boolean;
	/** Telemetry settings */
	telemetry: TelemetrySettings;
	/** Feature flags that both the UI and the backend can see */
	featureFlags: FeatureFlags;
	/** Settings related to fetching */
	fetch: Fetch;
	/** Settings related to Claude Code */
	claude: Claude;
	/** Settings related to code reviews and pull requests */
	reviews: Reviews;
	/** UI settings */
	ui: UiSettings;
	/** CLI settings */
	cli: Cli;
};

export type ForgeIntegrations = {
	/** Settings related to GitHub integration */
	github: GitHubSettings;
};

export type GitHubSettings = {
	/** The list of known GitHub users. This tracks the users that have authenticated through the application.
	 * That does not mean that the user is currently "active" in the app, just that they have authenticated at some point.
	 */
	knownUsernames: string[];
};

export type TelemetrySettings = {
	/** Whether the anonymous metrics are enabled. */
	appMetricsEnabled: boolean;
	/** Whether anonymous error reporting is enabled. */
	appErrorReportingEnabled: boolean;
	/** Whether non-anonymous metrics are enabled. */
	appNonAnonMetricsEnabled: boolean;
	/** Distinct ID, if reporting is enabled. */
	appDistinctId: string | null;
};

export type FeatureFlags = {
	/** Enable everything next-gen checkout */
	cv3: boolean;
	/** Use the V3 version of apply and unapply. */
	apply3: boolean;
	/** Enable processing of workspace rules. */
	rules: boolean;
	/** Enable single-branch mode. */
	singleBranch: boolean;
};

export type Fetch = {
	/** The frequency at which the app will automatically fetch. A negative value (e.g. -1) disables auto fetching. */
	autoFetchIntervalMinutes: number;
};

export type Claude = {
	/** Path to the Claude Code executable. Defaults to "claude" if not set. */
	executable: string;
	/** Whether to show notifications when Claude Code finishes. */
	notifyOnCompletion: boolean;
	/** Whether to show notifications when Claude Code needs permission. */
	notifyOnPermissionRequest: boolean;
	/** Whether to dangerously allow all permissions without prompting. */
	dangerouslyAllowAllPermissions: boolean;
	/** Whether to automatically commit changes and rename branches after completion. */
	autoCommitAfterCompletion: boolean;
	/** Whether to use the configured model in .claude/settings.json instead of passing --model. */
	useConfiguredModel: boolean;
};

export type Reviews = {
	/** Whether to auto-fill PR title and description from the first commit when a branch has only one commit. */
	autoFillPrDescriptionFromCommit: boolean;
};

export type UiSettings = {
	/** Whether to use the native system title bar. */
	useNativeTitleBar: boolean;
	/** Whether the `but` CLI is managed by a package manager.
	    When true, the UI should show a specific message instead of installation options. */
	cliIsManagedByPackageManager: false;
};
