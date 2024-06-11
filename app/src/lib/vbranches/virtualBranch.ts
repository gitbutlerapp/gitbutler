import { Branch, Commit, RemoteCommit, VirtualBranches, commitCompare } from './types';
import { invoke, listen } from '$lib/backend/ipc';
import { observableToStore } from '$lib/rxjs/store';
import { getRemoteBranchData } from '$lib/stores/remoteBranches';
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
				for (let i = 0; i < branches.length; i++) {
					const branch = branches[i];
					const commits = branch.commits;
					linkAsParentChildren(commits);
				}
			}),
			tap((branches) => {
				branches.forEach((branch) => {
					branch.files.sort((a) => (a.conflicted ? -1 : 0));
					// This is always true now
					branch.isMergeable = Promise.resolve(true);
				});
				this.fresh$.next(); // Notification for fresh reload
			}),
			startWith(undefined),
			shareReplay(1)
		);

		this.stashedBranches$ = this.branches$.pipe(
			map((branches) => branches?.filter((b) => !b.active))
		);

		// We need upstream data to be part of the branch without delay since the way we render
		// commits depends on it.
		// TODO: Move this async behavior into the rust code.
		this.activeBranches$ = this.branches$.pipe(
			// Disabling lint since `switchMap` does not work with async functions.
			// eslint-disable-next-line @typescript-eslint/promise-function-async
			switchMap((branches) => {
				if (!branches) return of();
				return Promise.all(
					branches
						.filter((b) => b.active)
						.map(async (b) => {
							const upstreamName = b.upstream?.name;
							if (upstreamName) {
								const data = await getRemoteBranchData(projectId, upstreamName);
								const commits = data.commits;
								commits.forEach((uc) => {
									const match = b.commits.find((c) => commitCompare(uc, c));
									if (match) {
										match.relatedTo = uc;
										uc.relatedTo = match;
									}
								});
								linkAsParentChildren(commits);
								b.upstreamData = data;
							}
							return b;
						})
				);
			})
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
				map((branches) => branches?.find((b) => b.id === branchId && b.upstream))
			)
		);
	}

	async getByUpstreamSha(upstreamSha: string) {
		return await firstValueFrom(
			this.branches$.pipe(
				timeout(10000),
				map((branches) => branches?.find((b) => b.upstream?.sha === upstreamSha))
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

function linkAsParentChildren(commits: Commit[] | RemoteCommit[]) {
	for (let j = 0; j < commits.length; j++) {
		const commit = commits[j];
		if (j === 0) {
			commit.next = undefined;
		} else {
			const child = commits[j - 1];
			if (child instanceof Commit) commit.next = child;
			if (child instanceof RemoteCommit) commit.next = child;
		}
		if (j !== commits.length - 1) {
			commit.prev = commits[j + 1];
		}
	}
}
