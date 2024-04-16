import { Branch, VirtualBranches } from './types';
import { invoke, listen } from '$lib/backend/ipc';
import { observableToStore } from '$lib/rxjs/store';
import * as toasts from '$lib/utils/toasts';
import { plainToInstance } from 'class-transformer';
import {
	switchMap,
	Observable,
	shareReplay,
	catchError,
	BehaviorSubject,
	concat,
	from,
	tap,
	map,
	firstValueFrom,
	timeout,
	of,
	startWith,
	Subject
} from 'rxjs';
import { writable, type Readable } from 'svelte/store';

export class VirtualBranchService {
	branches$: Observable<Branch[] | undefined>;
	stashedBranches$: Observable<Branch[] | undefined>;
	activeBranches$: Observable<Branch[] | undefined>;
	branchesError = writable<any>();
	private reload$ = new BehaviorSubject<void>(undefined);
	private fresh$ = new Subject<void>();

	activeBranches: Readable<Branch[] | undefined>;
	activeBranchesError: Readable<any>;

	constructor(projectId: string, gbBranchActive$: Observable<boolean>) {
		this.branches$ = this.reload$.pipe(
			switchMap(() => gbBranchActive$),
			switchMap((gbBranchActive) =>
				gbBranchActive
					? concat(
							from(listVirtualBranches({ projectId })),
							new Observable<Branch[]>((subscriber) => {
								return subscribeToVirtualBranches(projectId, (branches) =>
									subscriber.next(branches)
								);
							})
						)
					: of([])
			),
			tap((branches) => {
				branches.forEach((branch) => {
					branch.files.sort((a) => (a.conflicted ? -1 : 0));
					branch.isMergeable = invoke<boolean>('can_apply_virtual_branch', {
						projectId: projectId,
						branchId: branch.id
					});
				});
				this.fresh$.next(); // Notification for fresh reload
			}),
			startWith(undefined),
			shareReplay(1)
		);

		this.stashedBranches$ = this.branches$.pipe(
			map((branches) => branches?.filter((b) => !b.active))
		);

		this.activeBranches$ = this.branches$.pipe(
			map((branches) => branches?.filter((b) => b.active))
		);

		[this.activeBranches, this.activeBranchesError] = observableToStore(this.activeBranches$);
	}

	async reload() {
		this.branchesError.set(undefined);
		const fresh = firstValueFrom(
			this.fresh$.pipe(
				timeout(30000),
				catchError(() => {
					// Observable never errors for any other reasons
					const err = 'Timed out while reloading virtual branches';
					console.warn(err);
					toasts.error(err);
					return of();
				})
			)
		);
		this.reload$.next();
		return await fresh;
	}

	async getById(branchId: string) {
		return await firstValueFrom(
			this.branches$.pipe(
				timeout(10000),
				map((branches) => branches?.find((b) => b.id == branchId && b.upstream))
			)
		);
	}
}

function subscribeToVirtualBranches(projectId: string, callback: (branches: Branch[]) => void) {
	return listen<any>(`project://${projectId}/virtual-branches`, (event) =>
		callback(plainToInstance(VirtualBranches, event.payload).branches)
	);
}

export async function listVirtualBranches(params: { projectId: string }): Promise<Branch[]> {
	return plainToInstance(VirtualBranches, await invoke<any>('list_virtual_branches', params))
		.branches;
}
