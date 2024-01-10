import { BaseBranch, Branch } from './types';
import { plainToInstance } from 'class-transformer';
import { UserError, invoke, listen } from '$lib/backend/ipc';
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
				this.branchesError$.next(UserError.fromError(err));
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
	base$: Observable<BaseBranch | null>;
	error$ = new BehaviorSubject<any>(undefined);
	private reload$ = new BehaviorSubject<void>(undefined);

	constructor(projectId: string, fetches$: Observable<unknown>, head$: Observable<string>) {
		this.base$ = combineLatest([fetches$, head$, this.reload$]).pipe(
			debounceTime(100),
			switchMap(() => getBaseBranch({ projectId })),
			catchError((e) => {
				this.error$.next(e);
				throw e;
			}),
			shareReplay(1)
		);
	}

	reload() {
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
