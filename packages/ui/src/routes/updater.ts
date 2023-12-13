import { checkUpdate, installUpdate } from '@tauri-apps/api/updater';
import { BehaviorSubject, switchMap, type Observable, from, map, shareReplay } from 'rxjs';

export type Update = { enabled: boolean; shouldUpdate?: boolean; body?: string; version?: string };

export class UpdaterService {
	update$: Observable<Update>;
	private reload$ = new BehaviorSubject<void>(undefined);
	constructor() {
		this.update$ = this.reload$.pipe(
			switchMap(() => from(checkUpdate())),
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

		// Check for updates every 12h
		setInterval(() => {
			this.reload$.next();
		}, 43200 * 1000);
	}
}

export async function install() {
	await installUpdate();
}
