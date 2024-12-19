import { listen } from '$lib/backend/ipc';
import { plainToInstance } from 'class-transformer';
import { writable } from 'svelte/store';

export class SettingsService {
	// TODO: This is for testing purposes only
	readonly settings = writable<AppSettings | undefined>(undefined, () => {
		const unsubscribe = this.subscribe(async (settings) => console.log(settings));
		return () => {
			unsubscribe();
		};
	});

	constructor() {}

	private subscribe(callback: (settings: AppSettings) => void) {
		return listen<any>(`settings://update`, (event) =>
			callback(plainToInstance(AppSettings, event.payload))
		);
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
