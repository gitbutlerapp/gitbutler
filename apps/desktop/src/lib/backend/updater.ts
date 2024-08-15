import { listen } from './ipc';
import { showToast } from '$lib/notifications/toasts';
import { getVersion } from '@tauri-apps/api/app';
import { relaunch } from '@tauri-apps/api/process';
import {
	checkUpdate,
	installUpdate,
	onUpdaterEvent,
	type UpdateResult,
	type UpdateManifest,
	type UpdateStatus
} from '@tauri-apps/api/updater';
import posthog from 'posthog-js';
import { derived, writable, type Readable } from 'svelte/store';

// TODO: Investigate why 'DOWNLOADED' is not in the type provided by Tauri.
export type Update =
	| { version?: string; status?: UpdateStatus | 'DOWNLOADED'; body?: string }
	| undefined;

export class UpdaterService {
	private status = writable<UpdateStatus>(undefined);
	private result = writable<UpdateManifest | undefined>(undefined, () => {
		this.start();
		return () => {
			this.stop();
		};
	});

	update: Readable<Update | undefined> = derived(
		[this.status, this.result],
		([status, update]) => {
			return { ...update, status };
		},
		undefined
	);

	currentVersion = writable<string | undefined>(undefined);
	readonly version = derived(this.update, (update) => update?.version);

	prev: Update | undefined;
	unlistenStatusFn: any;
	unlistenManualCheckFn: any;
	intervalId: any;

	constructor() {}

	private async start() {
		const currentVersion = await getVersion();
		this.currentVersion.set(currentVersion);
		this.unlistenManualCheckFn = listen<string>('menu://global/update/clicked', () => {
			this.checkForUpdate(true);
		});

		this.unlistenStatusFn = await onUpdaterEvent((status) => {
			const err = status.error;
			if (err) {
				showErrorToast(err);
				posthog.capture('App Update Status Error', { error: err });
			}
			this.status.set(status.status);
		});

		await this.checkForUpdate();
		setInterval(async () => await this.checkForUpdate(), 3600000); // hourly
	}

	private async stop() {
		this.unlistenStatusFn?.();
		this.unlistenManualCheckFn?.();
		if (this.intervalId) {
			clearInterval(this.intervalId);
			this.intervalId = undefined;
		}
	}

	async checkForUpdate(isManual = false) {
		const update = await Promise.race([
			checkUpdate(), // In DEV mode this never returns.
			new Promise<UpdateResult>((resolve) =>
				setTimeout(() => resolve({ shouldUpdate: false }), 30000)
			)
		]);
		if (!update.shouldUpdate && isManual) {
			this.status.set('UPTODATE');
		} else if (update.manifest) {
			this.result.set(update.manifest);
		}
	}

	async installUpdate() {
		try {
			await installUpdate();
			posthog.capture('App Update Successful');
		} catch (err: any) {
			// We expect toast to be shown by error handling in `onUpdaterEvent`
			posthog.capture('App Update Install Error', { error: err });
		}
	}

	relaunchApp() {
		relaunch();
	}
}

function isOffline(err: any): boolean {
	return typeof err === 'string' && err.includes('Could not fetch a valid release');
}

function showErrorToast(err: any) {
	if (isOffline(err)) return;
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
