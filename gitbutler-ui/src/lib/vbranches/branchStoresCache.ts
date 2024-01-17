import * as toasts from '$lib/utils/toasts';

import { BaseBranch, Branch } from './types';
import { plainToInstance } from 'class-transformer';
import { invoke, listen } from '$lib/backend/ipc';
import {
	switchMap,
	Observable,
	shareReplay,
	catchError,
	BehaviorSubject,
	debounceTime,
	concat,
	from,
	tap,
	map,
	firstValueFrom,
	timeout,
	combineLatest,
	of
} from 'rxjs';

export class VirtualBranchService {
	branches$: Observable<Branch[]>;
	stashedBranches$: Observable<Branch[]>;
	activeBranches$: Observable<Branch[]>;
	branchesError$ = new BehaviorSubject<any>(undefined);
	private reload$ = new BehaviorSubject<void>(undefined);

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
			}),
			shareReplay(1),
			catchError((err) => {
				this.branchesError$.next(err);
				return [];
			})
		);

		this.stashedBranches$ = this.branches$.pipe(
			map((branches) => branches.filter((b) => !b.active))
		);

		this.activeBranches$ = this.branches$.pipe(map((branches) => branches.filter((b) => b.active)));
	}

	reload() {
		this.branchesError$.next(undefined);
		this.reload$.next();
	}

	async getById(branchId: string) {
		return await firstValueFrom(
			this.branches$.pipe(
				timeout(10000),
				map((branches) => branches.find((b) => b.id == branchId && b.upstream))
			)
		);
	}
}

function subscribeToVirtualBranches(projectId: string, callback: (branches: Branch[]) => void) {
	return listen<any[]>(`project://${projectId}/virtual-branches`, (event) =>
		callback(plainToInstance(Branch, event.payload))
	);
}

export class BaseBranchService {
	base$: Observable<BaseBranch | null | undefined>;
	busy$ = new BehaviorSubject(false);
	error$ = new BehaviorSubject<any>(undefined);
	private reload$ = new BehaviorSubject<void>(undefined);

	constructor(
		private projectId: string,
		fetches$: Observable<unknown>,
		head$: Observable<string>
	) {
		this.base$ = combineLatest([fetches$, head$, this.reload$]).pipe(
			debounceTime(100),
			switchMap(async () => {
				this.busy$.next(true);
				return await getBaseBranch({ projectId });
			}),
			tap(() => this.busy$.next(false)),
			shareReplay(1),
			catchError((e) => {
				this.error$.next(e);
				return of(undefined);
			})
		);
	}

	async fetchFromTarget() {
		this.busy$.next(true);
		try {
			// Note that we expect the back end to emit new fetches event, and therefore
			// trigger a base branch reload. It feels a bit awkward and should be improved.
			await invoke<void>('fetch_from_target', { projectId: this.projectId });
		} catch (err: any) {
			if (err.code === 'errors.git.authentication') {
				toasts.error('Failed to authenticate. Did you setup GitButler ssh keys?');
			} else {
				toasts.error(`Failed to fetch branch: ${err.message}`);
			}
		}
	}

	async setTarget(branch: string) {
		this.busy$.next(true);
		await invoke<BaseBranch>('set_base_branch', { projectId: this.projectId, branch });
		await this.fetchFromTarget();
	}

	reload() {
		this.busy$.next(true);
		this.reload$.next();
	}
}

export async function listVirtualBranches(params: { projectId: string }): Promise<Branch[]> {
	return plainToInstance(Branch, await invoke<any[]>('list_virtual_branches', params));
}

export async function getRemoteBranches(projectId: string | undefined) {
	if (!projectId) return [];
	return await invoke<Array<string>>('git_remote_branches', { projectId });
}

async function getBaseBranch(params: { projectId: string }): Promise<BaseBranch | null> {
	return plainToInstance(BaseBranch, await invoke<any>('get_base_branch_data', params));
}
