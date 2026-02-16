import { InjectionToken } from "@gitbutler/core/context";
import { writable } from "svelte/store";
import type { IBackend } from "$lib/backend";
import type { Settings } from "@gitbutler/core/api";

export const SETTINGS_SERVICE = new InjectionToken<SettingsService>("SettingsService");

export class SettingsService {
	readonly appSettings = writable<Settings.AppSettings | undefined>(undefined, () => {
		this.fetchAppSettings();
		const unsubscribe = this.listen(async (settings) => await this.handlePayload(settings));
		return () => {
			unsubscribe();
		};
	});

	readonly subscribe = this.appSettings.subscribe;

	constructor(private backend: IBackend) {}

	private async handlePayload(settings: Settings.AppSettings) {
		this.appSettings.set(settings);
	}

	/**
	 * Fetches the application settings from the backend & stores them in the local store.
	 */
	async fetchAppSettings(): Promise<Settings.AppSettings> {
		const settings = await this.backend.invoke<Settings.AppSettings>("get_app_settings");
		this.handlePayload(settings);
		return settings;
	}

	private listen(callback: (settings: Settings.AppSettings) => void) {
		return this.backend.listen<Settings.AppSettings>(`settings://update`, (event) =>
			callback(event.payload),
		);
	}

	async updateOnboardingComplete(update: boolean) {
		await this.invokeAndRefresh("update_onboarding_complete", { update });
	}

	async updateTelemetry(update: Partial<Settings.AppSettings["telemetry"]>) {
		await this.invokeAndRefresh("update_telemetry", { update });
	}

	async updateTelemetryDistinctId(appDistinctId: string | null) {
		await this.invokeAndRefresh("update_telemetry_distinct_id", { appDistinctId });
	}

	async updateFeatureFlags(update: Partial<Settings.AppSettings["featureFlags"]>) {
		await this.invokeAndRefresh("update_feature_flags", { update });
	}

	async updateClaude(update: Partial<Settings.AppSettings["claude"]>) {
		await this.invokeAndRefresh("update_claude", { update });
	}

	async updateReviews(update: Partial<Settings.AppSettings["reviews"]>) {
		await this.invokeAndRefresh("update_reviews", { update });
	}

	async updateFetch(update: Partial<Settings.AppSettings["fetch"]>) {
		await this.invokeAndRefresh("update_fetch", { update });
	}

	async updateUi(update: Partial<Settings.AppSettings["ui"]>) {
		await this.invokeAndRefresh("update_ui", { update });
	}

	/**
	 * For all projects this call deletes the following:
	 * - project meta data directory
	 * - project data directory
	 */
	async deleteAllData() {
		await this.invokeAndRefresh<void>("delete_all_data");
	}

	private async invokeAndRefresh<T>(command: string, ...args: any[]): Promise<T> {
		const result = await this.backend.invoke<T>(command, ...args);
		await this.fetchAppSettings();
		return result;
	}
}
