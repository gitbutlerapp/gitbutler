import { showToast } from '$lib/notifications/toasts';
import { InjectionToken } from '@gitbutler/shared/context';
import { get, writable } from 'svelte/store';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type {
	DownloadEvent,
	DownloadEventName,
	DownloadUpdate,
	IBackend,
	InstallUpdate,
	Update
} from '$lib/backend';
import type { ShortcutService } from '$lib/shortcuts/shortcutService';

export const UPDATER_SERVICE = new InjectionToken<UpdaterService>('UpdaterService');

type UpdateStatus = {
	version?: string;
	releaseNotes?: string;
	status?: InstallStatus | undefined;
};

export type InstallStatus =
	| 'Checking'
	| 'Downloading'
	| 'Downloaded'
	| 'Installing'
	| 'Done'
	| 'Up-to-date'
	| 'Error';

const downloadStatusMap: { [K in DownloadEventName]: InstallStatus } = {
	Started: 'Downloading',
	Progress: 'Downloading',
	Finished: 'Downloaded'
};

export const UPDATE_INTERVAL_MS = 3600000; // Hourly

/**
 * Note that the Tauri API `checkUpdate` hangs indefinitely in dev mode, build
 * a nightly if you want to test the updater manually.
 *
 * export TAURI_SIGNING_PRIVATE_KEY=doesnot
 * export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=matter
 * ./scripts/release.sh --channel nightly --version "0.5.678"
 */
export class UpdaterService {
	readonly disableAutoChecks = writable(false);
	readonly loading = writable(false);
	readonly update = writable<UpdateStatus>({}, () => {
		this.start();
		return () => {
			this.stop();
		};
	});

	private manualCheck = false;
	private checkForUpdateInterval: ReturnType<typeof setInterval> | undefined;
	private seenVersion: string | undefined;
	private backendDownload: DownloadUpdate | undefined;
	private backendInstall: InstallUpdate | undefined;

	unlistenStatus?: () => void;
	unlistenMenu?: () => void;

	constructor(
		private backend: IBackend,
		private posthog: PostHogWrapper,
		private shortcuts: ShortcutService
	) {}

	private async start() {
		// This shortcut registration is never unsubscribed, but that's likely
		// fine for the time being since the `AppUpdater` can never unmount.
		this.shortcuts.on('update', () => {
			this.checkForUpdate(true);
		});
		this.checkForUpdateInterval = setInterval(
			async () => await this.checkForUpdate(),
			UPDATE_INTERVAL_MS
		);
		this.checkForUpdate();
	}

	private async stop() {
		this.unlistenStatus?.();
		if (this.checkForUpdateInterval !== undefined) {
			clearInterval(this.checkForUpdateInterval);
			this.checkForUpdateInterval = undefined;
		}
	}

	async checkForUpdate(manual = false) {
		if (get(this.disableAutoChecks)) return;

		if (manual) {
			this.manualCheck = manual;
		}

		this.loading.set(true);
		try {
			this.handleUpdate(await this.backend.checkUpdate()); // In DEV mode this never returns.
		} catch (err: unknown) {
			handleError(err, manual);
		} finally {
			this.loading.set(false);
		}
	}

	private handleUpdate(update: Update | null) {
		if (update === null) {
			if (this.manualCheck) {
				this.setStatus('Up-to-date');
				return;
			}

			this.update.set({});
			return;
		}

		if (
			update.version !== this.seenVersion &&
			update.currentVersion !== '0.0.0' // DEV mode.
		) {
			this.backendDownload = async (onEvent) => await update.download(onEvent);
			this.backendInstall = async () => await update.install();

			this.seenVersion = update.version;
			this.update.set({
				version: update.version,
				releaseNotes: update.body,
				status: undefined
			});
		}
	}

	async downloadAndInstall() {
		this.loading.set(true);
		try {
			await this.download();
			await this.install();
			this.posthog.capture('App Update Successful');
		} catch (error: any) {
			// We expect toast to be shown by error handling in `onUpdaterEvent`
			handleError(error, true);
			this.update.set({ status: 'Error' });
			this.posthog.capture('App Update Install Error', { error });
		} finally {
			this.loading.set(false);
		}
	}

	private async download() {
		if (!this.backendDownload) {
			throw new Error('Download function not available.');
		}
		this.setStatus('Downloading');
		await this.backendDownload((progress: DownloadEvent) => {
			this.setStatus(downloadStatusMap[progress.event]);
		});
		this.setStatus('Downloaded');
	}

	private async install() {
		if (!this.backendInstall) {
			throw new Error('Install function not available.');
		}
		this.setStatus('Installing');
		await this.backendInstall();
		this.setStatus('Done');
	}

	private setStatus(status: InstallStatus) {
		this.update.update((update) => {
			return { ...update, status };
		});
	}

	async relaunchApp() {
		try {
			await this.backend.relaunch();
		} catch (err: unknown) {
			handleError(err, true);
		}
	}

	dismiss() {
		this.update.set({});
		this.manualCheck = false;
	}
}

function isOffline(err: any): boolean {
	return (
		typeof err === 'string' &&
		(err.includes('Could not fetch a valid release') ||
			err.includes('error sending request') ||
			err.includes('Network Error'))
	);
}

function handleError(err: any, manual: boolean) {
	if (!manual && isOffline(err)) return;
	console.error(err);
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
