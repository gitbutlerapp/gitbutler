import { listen, invoke } from '$lib/backend/ipc';
import { writable } from 'svelte/store';
import type { Tauri } from '$lib/backend/tauri';
import type { ProjectsService } from '$lib/project/projectsService';

export class SettingsService {
	readonly appSettings = writable<AppSettings | undefined>(undefined, () => {
		this.refresh();
		const unsubscribe = this.subscribe(async (settings) => await this.handlePayload(settings));
		return () => {
			unsubscribe();
		};
	});

	constructor(
		private tauri: Tauri,
		private projectsService: ProjectsService
	) {}

	private async handlePayload(settings: AppSettings) {
		this.appSettings.set(settings);
	}

	async refresh() {
		const response = await invoke<AppSettings>('get_app_settings');
		const settings = response;
		this.handlePayload(settings);
	}

	private subscribe(callback: (settings: AppSettings) => void) {
		return listen<AppSettings>(`settings://update`, (event) => callback(event.payload));
	}

	async updateOnboardingComplete(update: boolean) {
		await invoke('update_onboarding_complete', { update });
	}

	async updateTelemetry(update: Partial<TelemetrySettings>) {
		await invoke('update_telemetry', { update });
	}

	async updateFeatureFlags(update: Partial<FeatureFlags>) {
		// Doing a call to list_virtual_branches first to ensure the stack.tree properties are updated
		await invoke<any>('list_virtual_branches', {
			projectId: this.projectsService.getLastOpenedProject()
		});
		await invoke('update_feature_flags', { update });
	}

	/**
	 * For all projects this call deletes the following:
	 * - project meta data directory
	 * - project data directory
	 */
	async deleteAllData() {
		await this.tauri.invoke<void>('delete_all_data');
	}
}

export type AppSettings = {
	/** Whether the user has passed the onboarding flow. */
	onboardingComplete: boolean;
	/** Telemetry settings */
	telemetry: TelemetrySettings;
	/** Feature flags that both the UI and the backend can see */
	featureFlags: FeatureFlags;
};

export type TelemetrySettings = {
	/** Whether the anonymous metrics are enabled. */
	appMetricsEnabled: boolean;
	/** Whether anonymous error reporting is enabled. */
	appErrorReportingEnabled: boolean;
	/** Whether non-anonymous metrics are enabled. */
	appNonAnonMetricsEnabled: boolean;
};

export type FeatureFlags = {
	/** Enables the v3 design, as well as the purgatory mode (no uncommitted diff ownership assignments). */
	v3: boolean;
};
