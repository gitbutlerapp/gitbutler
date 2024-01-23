import * as toasts from '$lib/utils/toasts';

import {
	checkUpdate,
	installUpdate,
	onUpdaterEvent,
	type UpdateStatus
} from '@tauri-apps/api/updater';
import {
	BehaviorSubject,
	switchMap,
	Observable,
	from,
	map,
	shareReplay,
	interval,
	timeout,
	catchError,
	of,
	startWith,
	combineLatestWith
} from 'rxjs';

export type Update = { version?: string; status?: UpdateStatus } | undefined;

export class UpdaterService {
	private reload$ = new BehaviorSubject<void>(undefined);
	private status$ = new BehaviorSubject<UpdateStatus | undefined>(undefined);

	update$: Observable<Update>;

	// We don't ever call this because the class is meant to be used as a singleton
	unlistenFn: any;

	constructor() {
		onUpdaterEvent((status) => {
			this.status$.next(status.status);
			if (status.error) {
				toasts.error(status.error);
			}
		}).then((unlistenFn) => (this.unlistenFn = unlistenFn));

		this.update$ = this.reload$.pipe(
			switchMap(() => interval(60 * 1000).pipe(startWith(0))),
			switchMap(() =>
				from(checkUpdate()).pipe(
					timeout(10000), // In dev mode the promise hangs indefinitely.
					catchError(() => of(undefined))
				)
			),
			map((update) => {
				if (update?.manifest) {
					return { version: update.manifest.version };
				}
				return undefined;
			}),
			// Combine with status
			combineLatestWith(this.status$),
			map(([update, status]) => {
				return { ...update, status };
			}),
			shareReplay(1)
		);
		// this.update$ = of({ version: '1.0.0', status: 'DONE' });
	}

	async install() {
		await installUpdate();
	}
}
