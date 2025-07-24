/**
 * This file contains functions for managing application settings.
 * Settings are persisted in <Application Data>/settings.json and are used by both the UI and the backend.
 *
 * TODO: Rewrite this to be an injectable object so we don't need `storeInstance`.
 */

import { InjectionToken } from '@gitbutler/shared/context';
import { Store } from '@tauri-apps/plugin-store';
import { writable, type Writable } from 'svelte/store';

type DiskWritable<T> = Writable<T> & { onDisk: () => Promise<T> };

export async function loadAppSettings() {
	const diskStore = await Store.load('settings.json', { autoSave: true });
	return new AppSettings(diskStore);
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

	constructor(private diskStore: Store) {
		this.appAnalyticsConfirmed = this.persisted(false, 'appAnalyticsConfirmed');
		this.appMetricsEnabled = this.persisted(true, 'appMetricsEnabled');
		this.appErrorReportingEnabled = this.persisted(true, 'appErrorReportingEnabled');
		this.appNonAnonMetricsEnabled = this.persisted(false, 'appNonAnonMetricsEnabled');
	}

	private persisted<T>(initial: T, key: string): Writable<T> & { onDisk: () => Promise<T> } {
		const diskStore = this.diskStore;
		const storeValueWithDefault = this.storeValueWithDefault.bind(this);

		const keySpecificStore = writable<T>(initial, (set) => {
			synchronize(set);
		});

		const subscribe = keySpecificStore.subscribe;

		async function setAndPersist(value: T, set: (value: T) => void) {
			diskStore?.set(key, value);
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
		const stored = (await this.diskStore?.get(key)) as T;
		return stored === null || stored === undefined ? initial : stored;
	}
}
