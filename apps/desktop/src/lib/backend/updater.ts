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
export type Update = {
	version?: string;
	status?: UpdateStatus | 'DOWNLOADED';
	body?: string;
	manual: boolean;
};

export class UpdaterService {
	// True if manually initiated check.
	private manual = writable(false);

	// An object rather than string to prevent unique deduplication.
	private status = writable<{ status: UpdateStatus } | undefined>(undefined);

	private manifest = writable<UpdateManifest | undefined>(undefined, () => {
		this.start();
		return () => {
			this.stop();
		};
	});

	update: Readable<Update | undefined> = derived(
		[this.status, this.manifest, this.manual],
		([status, result, manual]) => {
			// Do not emit when up-to-date unless manually initiated.
			if (status?.status === 'UPTODATE' && result && !manual) {
				return;
			}
			return { ...result, ...status, manual };
		},
		undefined
	);

	// Needed to reset dismissed modal when version changes.
	currentVersion = writable<string | undefined>(undefined);
	readonly version = derived(this.update, (update) => update?.version);

	intervalId: any;
	unlistenStatusFn: any;
	unlistenManualCheckFn: any;

	constructor() {}

	private async start() {
		this.currentVersion.set(await getVersion());
		this.unlistenManualCheckFn = listen<string>('menu://global/update/clicked', () => {
			this.checkForUpdate(true);
		});
		this.unlistenStatusFn = await onUpdaterEvent((event) => {
			const { error, status } = event;
			if (error) {
				showErrorToast(error);
				posthog.capture('App Update Status Error', { error });
			}
			this.status.set({ status });
		});
		setInterval(async () => await this.checkForUpdate(), 3600000); // hourly
		this.checkForUpdate();
	}

	private async stop() {
		this.unlistenStatusFn?.();
		this.unlistenManualCheckFn?.();
		if (this.intervalId) {
			clearInterval(this.intervalId);
			this.intervalId = undefined;
		}
	}

	async checkForUpdate(manual = false) {
		const update = await Promise.race([
			checkUpdate(), // In DEV mode this never returns.
			new Promise<UpdateResult>((resolve) =>
				setTimeout(() => resolve({ shouldUpdate: false }), 30000)
			)
		]);
		this.manual.set(manual);
		if (!update.shouldUpdate && manual) {
			this.status.set({ status: 'UPTODATE' });
		} else if (update.manifest) {
			this.manifest.set(update.manifest);
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
