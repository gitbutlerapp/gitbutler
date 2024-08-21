import { Tauri } from './tauri';
import { showToast } from '$lib/notifications/toasts';
import { relaunch } from '@tauri-apps/api/process';
import {
	installUpdate,
	type UpdateResult,
	type UpdateManifest,
	type UpdateStatus
} from '@tauri-apps/api/updater';
import posthog from 'posthog-js';
import { derived, readable, writable } from 'svelte/store';

type Status = UpdateStatus | 'DOWNLOADED';
const TIMEOUT_SECONDS = 30;

export const UPDATE_INTERVAL_MS = 3600000; // Hourly

/**
 * Note that the Tauri API `checkUpdate` hangs indefinitely in dev mode, build
 * a nightly if you want to test the updater manually.
 *
 * export TAURI_PRIVATE_KEY=doesnot
 * export TAURI_KEY_PASSWORD=matter
 * ./scripts/release.sh --channel nightly --version "0.5.678"
 */
export class UpdaterService {
	readonly loading = writable(false);
	readonly status = writable<Status | undefined>();
	private manifest = writable<UpdateManifest | undefined>(undefined, () => {
		this.start();
		return () => {
			this.stop();
		};
	});

	private currentVersion = readable<string | undefined>(undefined, (set) => {
		this.tauri.currentVersion().then((version) => set(version));
	});

	readonly update = derived(
		[this.manifest, this.status, this.currentVersion],
		([manifest, status, currentVersion]) => {
			return {
				version: manifest?.version,
				releaseNotes: manifest?.body,
				status,
				currentVersion
			};
		}
	);

	private intervalId: any;
	private seenVersion: string | undefined;

	unlistenStatus?: () => void;
	unlistenMenu?: () => void;

	constructor(private tauri: Tauri) {}

	private async start() {
		this.unlistenMenu = this.tauri.listen<string>('menu://global/update/clicked', () => {
			this.checkForUpdate(true);
		});

		this.unlistenStatus = await this.tauri.onUpdaterEvent((event) => {
			const { error, status } = event;
			this.status.set(status);
			if (error) {
				handleError(error, false);
				posthog.capture('App Update Status Error', { error });
			}
		});

		setInterval(async () => await this.checkForUpdate(), UPDATE_INTERVAL_MS);
		this.checkForUpdate();
	}

	private async stop() {
		this.unlistenStatus?.();
		this.unlistenMenu?.();

		if (this.intervalId) {
			clearInterval(this.intervalId);
			this.intervalId = undefined;
		}
	}

	async checkForUpdate(manual = false) {
		this.loading.set(true);
		try {
			const update = await Promise.race([
				this.tauri.checkUpdate(), // In DEV mode this never returns.
				new Promise<UpdateResult>((_resolve, reject) =>
					// For manual testing use resolve instead of reject here.
					setTimeout(
						() => reject(`Timed out after ${TIMEOUT_SECONDS} seconds.`),
						TIMEOUT_SECONDS * 1000
					)
				)
			]);
			await this.processUpdate(update, manual);
		} catch (err: unknown) {
			// No toast unless manually invoked.
			if (manual) {
				handleError(err, true);
			} else {
				console.error(err);
			}
		} finally {
			this.loading.set(false);
		}
	}

	private async processUpdate(update: UpdateResult, manual: boolean) {
		const { shouldUpdate, manifest } = update;
		if (shouldUpdate === false && manual) {
			this.status.set('UPTODATE');
		}
		if (manifest && manifest.version !== this.seenVersion) {
			this.manifest.set(manifest);
			this.seenVersion = manifest.version;
		}
	}

	async installUpdate() {
		this.loading.set(true);
		try {
			await installUpdate();
			posthog.capture('App Update Successful');
		} catch (err: any) {
			// We expect toast to be shown by error handling in `onUpdaterEvent`
			posthog.capture('App Update Install Error', { error: err });
		} finally {
			this.loading.set(false);
		}
	}

	async relaunchApp() {
		try {
			await relaunch();
		} catch (err: unknown) {
			handleError(err, true);
		}
	}

	dismiss() {
		this.manifest.set(undefined);
		this.status.set(undefined);
	}
}

function isOffline(err: any): boolean {
	return (
		typeof err === 'string' &&
		(err.includes('Could not fetch a valid release') || err.includes('Network Error'))
	);
}

function handleError(err: any, manual: boolean) {
	if (!manual && isOffline(err)) return;
	showToast({
		title: 'App update failed',
		message: `
            Something went wrong while updating the app.

            You can download the latest release from our
            [downloads](https://app.gitbutler.com/downloads) page.
        `,
		error: err,
		style: 'error'
	});
}
