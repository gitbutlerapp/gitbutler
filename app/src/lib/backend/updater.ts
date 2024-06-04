import { showToast } from '$lib/notifications/toasts';
import { relaunch } from '@tauri-apps/api/process';
import {
	checkUpdate,
	installUpdate,
	onUpdaterEvent,
	type UpdateResult,
	type UpdateStatus
} from '@tauri-apps/api/updater';
import posthog from 'posthog-js';
import {
	of,
	tap,
	map,
	from,
	timeout,
	interval,
	switchMap,
	shareReplay,
	catchError,
	startWith,
	combineLatestWith,
	distinctUntilChanged,
	Observable,
	BehaviorSubject
} from 'rxjs';

// TOOD: Investigate why 'DOWNLOADED' is not in the type provided by Tauri.
export type Update =
	| { version?: string; status?: UpdateStatus | 'DOWNLOADED'; body?: string }
	| undefined;

export class UpdaterService {
	private reload$ = new BehaviorSubject<void>(undefined);
	private status$ = new BehaviorSubject<UpdateStatus | undefined>(undefined);

	/**
	 * Example output:
	 * {version: "0.5.303", date: "2024-02-25 3:09:58.0 +00:00:00", body: "", status: "DOWNLOADED"}
	 */
	update$: Observable<Update>;

	// We don't ever call this because the class is meant to be used as a singleton
	unlistenFn: any;

	constructor() {
		onUpdaterEvent((status) => {
			const err = status.error;
			if (err) {
				showErrorToast(err);
				posthog.capture('App Update Status Error', { error: err });
			}
			this.status$.next(status.status);
		}).then((unlistenFn) => (this.unlistenFn = unlistenFn));

		this.update$ = this.reload$.pipe(
			// Now and then every hour indefinitely
			switchMap(() => interval(60 * 60 * 1000).pipe(startWith(0))),
			tap(() => this.status$.next(undefined)),
			// Timeout needed to prevent hanging in dev mode
			switchMap(() => from(checkUpdate()).pipe(timeout(10000))),
			// The property `shouldUpdate` seems useless, only indicates presence of manifest
			map((update: UpdateResult | undefined) => {
				if (update?.shouldUpdate) return update.manifest;
			}),
			// We don't need the stream to emit if the result is the same version
			distinctUntilChanged((prev, curr) => prev?.version === curr?.version),
			// Hide offline/timeout errors since no app ever notifies you about this
			catchError((err) => {
				if (!isOffline(err) && !isTimeoutError(err)) {
					posthog.capture('App Update Check Error', { error: err });
					showErrorToast(err);
					console.log(err);
				}
				return of(undefined);
			}),
			// Status is irrelevant without a proposed update so we merge the streams
			combineLatestWith(this.status$),
			map(([update, status]) => {
				if (update) return { ...update, status };
				return undefined;
			}),
			shareReplay(1)
		);
		// Use this for testing component manually (until we have actual tests)
		// this.update$ = new BehaviorSubject({
		// 	version: '0.5.303',
		// 	date: '2024-02-25 3:09:58.0 +00:00:00',
		// 	body: '- Improves the performance of virtual branch operations (quicker and lower CPU usage)\n- Large numbers of hunks for a file will only be rendered in the UI after confirmation'
		// });
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

function isTimeoutError(err: any): boolean {
	return err?.name === 'TimeoutError';
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
