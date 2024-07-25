import { showToast } from '$lib/notifications/toasts';
import { relaunch } from '@tauri-apps/api/process';
import {
	checkUpdate,
	installUpdate,
	onUpdaterEvent,
	type UpdateManifest,
	type UpdateStatus
} from '@tauri-apps/api/updater';
import posthog from 'posthog-js';
import { derived, writable, type Readable } from 'svelte/store';

// TOOD: Investigate why 'DOWNLOADED' is not in the type provided by Tauri.
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
			if (update) return { ...update, status };
			return undefined;
		},
		undefined
	);

	readonly version = derived(this.update, (update) => update?.version);

	prev: Update | undefined;
	unlistenFn: any;
	intervalId: any;

	constructor() {}

	private async start() {
		await this.checkForUpdate();
		setInterval(async () => await this.checkForUpdate(), 3600000); // hourly
		this.unlistenFn = await onUpdaterEvent((status) => {
			const err = status.error;
			if (err) {
				showErrorToast(err);
				posthog.capture('App Update Status Error', { error: err });
			}
			this.status.set(status.status);
		});
	}

	private async stop() {
		this.unlistenFn?.();
		if (this.intervalId) {
			clearInterval(this.intervalId);
			this.intervalId = undefined;
		}
	}

	async checkForUpdate() {
		const update = await checkUpdate();
		if (update.manifest) {
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
