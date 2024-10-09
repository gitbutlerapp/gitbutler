/**
 * This file contains functions for managing application settings.
 * Settings are persisted in <Application Data>/settings.json and are used by both the UI and the backend.
 *
 * TODO: Rewrite this to be an injectable object so we don't need `storeInstance`.
 */

import { createStore } from '@tauri-apps/plugin-store';
import { writable, type Writable } from 'svelte/store';

export class AppSettings {
	// TODO: Remove this once `autoSave: boolean` is upstreamed.
	// @ts-expect-error ts-2322
	diskStore = createStore('settings.json', { autoSave: 1 });
	constructor() {}

	/**
	 * Persisted confirmation that user has confirmed their analytics settings.
	 */
	readonly appAnalyticsConfirmed = this.persisted(false, 'appAnalyticsConfirmed');

	/**
	 * Provides a writable store for obtaining or setting the current state of application metrics.
	 * The application metrics can be enabled or disabled by setting the value of the store to true or false.
	 * @returns A writable store with the appMetricsEnabled config.
	 */
	readonly appMetricsEnabled = this.persisted(true, 'appMetricsEnabled');

	/**
	 * Provides a writable store for obtaining or setting the current state of application error reporting.
	 * The application error reporting can be enabled or disabled by setting the value of the store to true or false.
	 * @returns A writable store with the appErrorReportingEnabled config.
	 */
	readonly appErrorReportingEnabled = this.persisted(true, 'appErrorReportingEnabled');

	/**
	 * Provides a writable store for obtaining or setting the current state of non-anonemous application metrics.
	 * The setting can be enabled or disabled by setting the value of the store to true or false.
	 * @returns A writable store with the appNonAnonMetricsEnabled config.
	 */
	readonly appNonAnonMetricsEnabled = this.persisted(false, 'appNonAnonMetricsEnabled');

	private persisted<T>(initial: T, key: string): Writable<T> & { onDisk: () => Promise<T> } {
		const diskStore = this.diskStore;
		const storeValueWithDefault = this.storeValueWithDefault.bind(this);

		const keySpecificStore = writable<T>(initial, (set) => {
			synchronize(set);
		});

		const subscribe = keySpecificStore.subscribe;

		async function setAndPersist(value: T, set: (value: T) => void) {
			const store = await diskStore;
			store.set(key, value);
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
		const store = await this.diskStore;
		try {
			await store.load();
		} catch (e) {
			store.reset();
		}
		const stored = (await store.get(key)) as T;
		return stored === null ? initial : stored;
	}
}
