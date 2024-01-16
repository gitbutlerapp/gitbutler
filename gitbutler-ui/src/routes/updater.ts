import { checkUpdate, installUpdate } from '@tauri-apps/api/updater';
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
	startWith
} from 'rxjs';

export type Update = { enabled: boolean; shouldUpdate?: boolean; body?: string; version?: string };

export class UpdaterService {
	update$: Observable<Update>;
	private reload$ = new BehaviorSubject<void>(undefined);
	constructor() {
		this.update$ = this.reload$.pipe(
			switchMap(() => interval(6 * 60 * 60 * 1000).pipe(startWith(0))),
			switchMap(() =>
				from(checkUpdate()).pipe(
					timeout(30000), // In dev mode the promise hangs indefinitely.
					catchError(() => of({ shouldUpdate: false, manifest: undefined }))
				)
			),
			map((update) => {
				if (update === undefined) {
					return { enabled: false };
				} else if (!update.shouldUpdate) {
					return { enabled: true, shouldUpdate: false };
				} else {
					return {
						enabled: true,
						shouldUpdate: true,
						body: update.manifest!.body,
						version: update.manifest!.version
					};
				}
			}),
			shareReplay(1)
		);
		// this.update$ = of({ enabled: true, shouldUpdate: true, version: '1.0.0', body: 'blah' });
	}
}

export async function install() {
	await installUpdate();
}
