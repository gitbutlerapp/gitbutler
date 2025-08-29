import { InjectionToken } from '@gitbutler/shared/context';
import { getStorageItem, setStorageItem } from '@gitbutler/shared/persisted';
import { writable } from 'svelte/store';
import type { IBackend } from '$lib/backend';

export const SETTINGS_SERVICE = new InjectionToken<SettingsService>('SettingsService');

const WS3_AUTO_TOGGLE = 'ws3AutoToggle';
const RULES_AUTO_TOGGLE = 'rulesAutoToggle';

export class SettingsService {
	readonly appSettings = writable<AppSettings | undefined>(undefined, () => {
		this.refresh();
		const unsubscribe = this.listen(async (settings) => await this.handlePayload(settings));
		return () => {
			unsubscribe();
		};
	});

	readonly subscribe = this.appSettings.subscribe;

	constructor(private backend: IBackend) {
		this.nudge();
	}

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

	private async nudge(): Promise<void> {
		await this.autoOptInWs3();
		await this.autoOptInRules();
	}

	/**
	 * Automatically opt-in to WS3 if it is not already enabled.
	 *
	 * This is done only to kickstart the usage of WS3, so that users can try it out.
	 * Once the transition into WS3 is complete, this method can be removed.
	 */
	private async autoOptInWs3() {
		try {
			const response = await this.backend.invoke<AppSettings>('get_app_settings');
			const performedAutoToggle = getStorageItem(WS3_AUTO_TOGGLE) ?? false;
			// If the auto toggle has already been performed, we do not need to do it again.
			if (performedAutoToggle) return;
			if (response.featureFlags.ws3) {
				// If the WS3 feature flag is already enabled, set the flag and do not toggle it again.
				setStorageItem(WS3_AUTO_TOGGLE, true);
				return;
			}

			// If the WS3 feature flag is not enabled, we automatically enable it for the
			// first time, so that the user can try it out.
			await this.updateFeatureFlags({
				ws3: true
			});

			setStorageItem(WS3_AUTO_TOGGLE, true);
		} catch (error: unknown) {
			console.error(`Failed to auto-opt-in to WS3: ${error}`);
		}
	}

	/**
	 * Automatically opt-in to rules if it is not already enabled.
	 *
	 * This is done only to kickstart the usage of rules, so that users can try it out.
	 */
	private async autoOptInRules() {
		try {
			const response = await this.backend.invoke<AppSettings>('get_app_settings');
			const performedAutoToggle = getStorageItem(RULES_AUTO_TOGGLE) ?? false;
			// If the auto toggle has already been performed, we do not need to do it again.
			if (performedAutoToggle) return;
			if (response.featureFlags.rules) {
				// If the rules feature flag is already enabled, set the flag and do not toggle it again.
				setStorageItem(RULES_AUTO_TOGGLE, true);
				return;
			}

			// If the rules feature flag is not enabled, we automatically enable it for the
			// first time, so that the user can try it out.
			await this.updateFeatureFlags({
				rules: true
			});

			setStorageItem(RULES_AUTO_TOGGLE, true);
		} catch (error: unknown) {
			console.error(`Failed to auto-opt-in to rules: ${error}`);
		}
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
	/** Enable the usage of the V3 workspace API */
	ws3: boolean;
	/** Enable the usage of GitButler Acitions. */
	actions: boolean;
	/** Enable the usage of butbot chat */
	butbot: boolean;
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
};
