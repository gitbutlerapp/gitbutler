import { BaseBranch, Branch, RemoteBranch } from './types';
import { plainToInstance } from 'class-transformer';
import { UserError, invoke, listen } from '$lib/backend/ipc';
import {
	merge,
	switchMap,
	Observable,
	shareReplay,
	catchError,
	BehaviorSubject,
	debounceTime,
	concat,
	from,
	tap,
	map
} from 'rxjs';

export class VirtualBranchService {
	branches$: Observable<Branch[]>;
	stashedBranches$: Observable<Branch[]>;
	branchesError$ = new BehaviorSubject<any>(undefined);
	private reload$ = new BehaviorSubject<void>(undefined);

	constructor(projectId: string) {
		this.branches$ = this.reload$.pipe(
			switchMap(() =>
				concat(
					from(listVirtualBranches({ projectId })),
					new Observable<Branch[]>((subscriber) => {
						return subscribeToVirtualBranches(projectId, (branches) => subscriber.next(branches));
					})
				).pipe(
					tap((branches) => {
						branches.forEach((branch) => {
							branch.files.sort((a) => (a.conflicted ? -1 : 0));
							branch.isMergeable = invoke<boolean>('can_apply_virtual_branch', {
								projectId: projectId,
								branchId: branch.id
							});
						});
					}),
					catchError((err) => {
						this.branchesError$.next(UserError.fromError(err));
						return [];
					}),
					shareReplay(1)
				)
			)
		);

		this.stashedBranches$ = this.branches$.pipe(
			map((branches) => branches.filter((b) => !b.active))
		);
	}

	reload() {
		this.branchesError$.next(undefined);
		this.reload$.next();
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

	constructor(projectId: string, fetches$: Observable<void>, head$: Observable<string>) {
		this.base$ = merge(fetches$, head$, this.reload$).pipe(
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

export async function getRemoteBranchesData(params: {
	projectId: string;
}): Promise<RemoteBranch[]> {
	const branches = plainToInstance(
		RemoteBranch,
		await invoke<any[]>('list_remote_branches', params)
	);

	return branches;
}

export async function getRemoteBranches(projectId: string | undefined) {
	if (!projectId) return [];
	return await invoke<Array<string>>('git_remote_branches', { projectId });
}

async function getBaseBranch(params: { projectId: string }): Promise<BaseBranch | null> {
	const baseBranch = plainToInstance(BaseBranch, await invoke<any>('get_base_branch_data', params));
	if (baseBranch) {
		// The rust code performs a fetch when get_base_branch_data is invoked
		baseBranch.fetchedAt = new Date();
		return baseBranch;
	}
	return null;
}
