/**
 * This file contains functions for managing application settings.
 * Settings are persisted in <Application Data>/settings.json and are used by both the UI and the backend.
 *
 * TODO: Rewrite this to be an injectable object so we don't need `storeInstance`.
 */

import { InjectionToken } from '@gitbutler/core/context';
import { persisted } from '@gitbutler/shared/persisted';
import { get, writable, type Writable } from 'svelte/store';
import type { DiskStore, IBackend } from '$lib/backend';
import type { SettingsService } from '$lib/config/appSettingsV2';

type DiskWritable<T> = Writable<T> & { onDisk: () => Promise<T> };

export async function loadAppSettings(backend: IBackend, settingsService: SettingsService) {
	const diskStore = await backend.loadDiskStore('settings.json');
	return new AppSettings(diskStore, settingsService);
}

export const APP_SETTINGS = new InjectionToken<AppSettings>('AppSettings');

export class AppSettings {
	/**
	 * Persisted confirmation that user has confirmed their analytics settings.
	 */
	readonly appAnalyticsConfirmed: DiskWritable<boolean>;

	/**
	 * Provides a writable store for obtaining or setting the current state of application metrics.
	 * The application metrics can be enabled or disabled by setting the value of the store to true or false.
	 * @returns A writable store with the appMetricsEnabled config.
	 */
	readonly appMetricsEnabled: DiskWritable<boolean>;

	/**
	 * Provides a writable store for obtaining or setting the current state of application error reporting.
	 * The application error reporting can be enabled or disabled by setting the value of the store to true or false.
	 * @returns A writable store with the appErrorReportingEnabled config.
	 */
	readonly appErrorReportingEnabled: DiskWritable<boolean>;

	/**
	 * Provides a writable store for obtaining or setting the current state of non-anonemous application metrics.
	 * The setting can be enabled or disabled by setting the value of the store to true or false.
	 * @returns A writable store with the appNonAnonMetricsEnabled config.
	 */
	readonly appNonAnonMetricsEnabled: DiskWritable<boolean>;

	constructor(
		private diskStore: DiskStore,
		private settingsService: SettingsService
	) {
		this.appAnalyticsConfirmed = this.withDoubleWrite(
			this.persisted(false, 'appAnalyticsConfirmed'),
			async (value) => {
				await this.settingsService.updateOnboardingComplete(value);
			}
		);
		this.appMetricsEnabled = this.withDoubleWrite(
			this.persisted(true, 'appMetricsEnabled'),
			async (value) => {
				await this.settingsService.updateTelemetry({ appMetricsEnabled: value });
			}
		);
		this.appErrorReportingEnabled = this.withDoubleWrite(
			this.persisted(true, 'appErrorReportingEnabled'),
			async (value) => {
				await this.settingsService.updateTelemetry({ appErrorReportingEnabled: value });
			}
		);
		this.appNonAnonMetricsEnabled = this.withDoubleWrite(
			this.persisted(false, 'appNonAnonMetricsEnabled'),
			async (value) => {
				await this.settingsService.updateTelemetry({ appNonAnonMetricsEnabled: value });
			}
		);
	}

	private withDoubleWrite<T>(
		store: DiskWritable<T>,
		onWrite: (value: T) => Promise<void>
	): DiskWritable<T> {
		const originalSet = store.set;
		return {
			...store,
			set: async (value: T) => {
				originalSet(value);
				// Double-write must succeed to prevent settings desync during migration
				// If this fails, we want to throw so the caller knows the write didn't fully complete
				await onWrite(value);
			}
		};
	}

	private persisted<T>(initial: T, key: string): Writable<T> & { onDisk: () => Promise<T> } {
		if (!this.diskStore) {
			const writable = persisted(initial, key);
			return {
				...writable,
				onDisk: async () => await Promise.resolve(get(writable))
			};
		}

		const diskStore = this.diskStore;
		const storeValueWithDefault = this.storeValueWithDefault.bind(this);

		const keySpecificStore = writable<T>(initial, (set) => {
			synchronize(set);
		});

		const subscribe = keySpecificStore.subscribe;

		async function setAndPersist(value: T, set: (value: T) => void) {
			diskStore.set(key, value);
			set(value);
		}

		async function synchronize(set: (value: T) => void): Promise<void> {
			const value = await storeValueWithDefault(initial, key);
			set(value);
		}

		async function set(value: T) {
			setAndPersist(value, keySpecificStore.set);
		}

		async function onDisk() {
			return await storeValueWithDefault(initial, key);
		}

		function update() {
			throw 'Not implemented';
		}

		return { subscribe, set, update, onDisk };
	}

	async storeValueWithDefault<T>(initial: T, key: string): Promise<T> {
		const stored = await this.diskStore.get<T>(key);
		return stored === null || stored === undefined ? initial : stored;
	}
}
