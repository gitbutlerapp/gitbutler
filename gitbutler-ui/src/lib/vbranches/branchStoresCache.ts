import { BaseBranch, Branch, VirtualBranches } from './types';
import { Code, invoke, listen } from '$lib/backend/ipc';
import { observableToStore } from '$lib/rxjs/store';
import * as toasts from '$lib/utils/toasts';
import { plainToInstance } from 'class-transformer';
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

	constructor(
		private projectId: string,
		gbBranchActive$: Observable<boolean>
	) {
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
			shareReplay(1),
			catchError((err) => {
				this.branchesError.set(err);
				return [];
			})
		);

		this.stashedBranches$ = this.branches$.pipe(
			map((branches) => branches?.filter((b) => !b.active))
		);

		this.activeBranches$ = this.branches$.pipe(
			map((branches) => branches?.filter((b) => b.active))
		);
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

export class NoDefaultTarget extends Error {}

export class BaseBranchService {
	readonly base$: Observable<BaseBranch | null | undefined>;
	readonly busy$ = new BehaviorSubject(false);
	readonly error$ = new BehaviorSubject<any>(undefined);
	private readonly reload$ = new BehaviorSubject<void>(undefined);

	readonly base: Readable<BaseBranch | null | undefined>;
	readonly error: Readable<any>;

	constructor(
		private readonly projectId: string,
		readonly remoteUrl$: BehaviorSubject<string | undefined>,
		fetches$: Observable<unknown>,
		readonly head$: Observable<string>
	) {
		this.base$ = combineLatest([fetches$, head$, this.reload$]).pipe(
			debounceTime(100),
			switchMap(async () => {
				this.busy$.next(true);
				const baseBranch = await getBaseBranch({ projectId });
				if (!baseBranch) {
					throw new NoDefaultTarget();
				}
				this.busy$.next(false);
				return baseBranch;
			}),
			tap((baseBranch) => {
				if (baseBranch?.remoteUrl) this.remoteUrl$.next(baseBranch.remoteUrl);
			}),
			catchError((e) => {
				this.remoteUrl$.next(undefined);
				this.busy$.next(false);
				throw e;
			}),
			// Start with undefined to prevent delay in updating $baseBranch value in
			// layout.svelte when navigating between projects.
			startWith(undefined),
			shareReplay(1)
		);
		[this.base, this.error] = observableToStore(this.base$);
	}

	async fetchFromTarget(action: string | undefined = undefined) {
		this.busy$.next(true);
		try {
			// Note that we expect the back end to emit new fetches event, and therefore
			// trigger a base branch reload. It feels a bit awkward and should be improved.
			await invoke<void>('fetch_from_target', {
				projectId: this.projectId,
				action: action || 'auto'
			});
		} catch (err: any) {
			if (err.message?.includes('does not have a default target')) {
				// Swallow this error since user should be taken to project setup page
				return;
			} else if (err.code === Code.ProjectsGitAuth) {
				toasts.error('Failed to authenticate. Did you setup GitButler ssh keys?');
			} else {
				toasts.error(`${err.message}`);
			}
			console.error(err);
		} finally {
			this.busy$.next(false);
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
	return plainToInstance(VirtualBranches, await invoke<any>('list_virtual_branches', params))
		.branches;
}

export async function getRemoteBranches(projectId: string | undefined) {
	if (!projectId) return [];
	return await invoke<Array<string>>('git_remote_branches', { projectId }).then((branches) =>
		branches
			.map((name) => name.substring(13))
			.sort((a, b) => a.localeCompare(b))
			.map((name) => ({ name }))
	);
}

async function getBaseBranch(params: { projectId: string }): Promise<BaseBranch | null> {
	return plainToInstance(BaseBranch, await invoke<any>('get_base_branch_data', params));
}
