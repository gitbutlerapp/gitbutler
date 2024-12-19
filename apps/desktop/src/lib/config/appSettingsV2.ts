import { listen, invoke } from '$lib/backend/ipc';
import { plainToInstance } from 'class-transformer';
import { writable } from 'svelte/store';

export class SettingsService {
	readonly settings = writable<AppSettings | undefined>(undefined, () => {
		this.refresh();
		const unsubscribe = this.subscribe(async (settings) => await this.handlePayload(settings));
		return () => {
			unsubscribe();
		};
	});

	private async handlePayload(settings: AppSettings) {
		// TODO: Remove this log
		console.log(settings);
		this.settings.set(settings);
	}

	private async refresh() {
		const response = await invoke<any>('get_app_settings');
		const settings = plainToInstance(AppSettings, response);
		this.handlePayload(settings);
	}

	private subscribe(callback: (settings: AppSettings) => void) {
		return listen<any>(`settings://update`, (event) =>
			callback(plainToInstance(AppSettings, event.payload))
		);
	}

	public async updateOnboardingComplete(update: boolean) {
		await invoke('update_onboarding_complete', { update });
	}

	public async updateTelemetry(update: TelemetryUpdate) {
		await invoke('update_telemetry', { update });
	}
}

export class AppSettings {
	/** Whether the user has passed the onboarding flow. */
	onboardingComplete!: boolean;
	/** Telemetry settings */
	telemetry!: TelemetrySettings;
}

export class TelemetrySettings {
	/** Whether the anonymous metrics are enabled. */
	appMetricsEnabled!: boolean;
	/** Whether anonymous error reporting is enabled. */
	appErrorReportingEnabled!: boolean;
	/** Whether non-anonymous metrics are enabled. */
	appNonAnonMetricsEnabled!: boolean;
}

/** Request updating the TelemetrySettings. Only the fields that are set are updated */
export class TelemetryUpdate {
	/** Whether the anonymous metrics are enabled. */
	appMetricsEnabled?: boolean | undefined;
	/** Whether anonymous error reporting is enabled. */
	appErrorReportingEnabled?: boolean | undefined;
	/** Whether non-anonymous metrics are enabled. */
	appNonAnonMetricsEnabled?: boolean | undefined;
}
