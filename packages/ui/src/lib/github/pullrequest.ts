import lscache from 'lscache';
import {
	Observable,
	EMPTY,
	BehaviorSubject,
	of,
	firstValueFrom,
	Subject,
	combineLatest
} from 'rxjs';
import {
	catchError,
	combineLatestWith,
	map,
	shareReplay,
	switchMap,
	tap,
	timeout
} from 'rxjs/operators';

import {
	type PullRequest,
	type GitHubIntegrationContext,
	ghResponseToInstance,
	type PrStatus
} from '$lib/github/types';
import { newClient } from '$lib/github/client';
import type { BranchController } from '$lib/vbranches/branchController';
import type { VirtualBranchService } from '$lib/vbranches/branchStoresCache';
import type { Octokit } from '@octokit/rest';

export type PrAction = 'creating_pr';
export type PrServiceState = { busy: boolean; branchId: string; action?: PrAction };

export class PrService {
	prs$: Observable<PullRequest[]>;
	error$ = new BehaviorSubject<string | undefined>(undefined);

	private stateMap = new Map<string, BehaviorSubject<PrServiceState>>();
	private reload$ = new BehaviorSubject<{ skipCache: boolean } | undefined>(undefined);
	private fresh$ = new Subject<void>();
	private octokit$: Observable<Octokit | undefined>;
	private active$ = new BehaviorSubject<boolean>(false);

	constructor(
		private branchController: BranchController,
		private branchService: VirtualBranchService,
		private ctx$: Observable<GitHubIntegrationContext | undefined>
	) {
		this.octokit$ = this.ctx$.pipe(map((ctx) => (ctx ? newClient(ctx) : undefined)));
		this.prs$ = ctx$.pipe(
			combineLatestWith(this.octokit$, this.reload$),
			tap(() => this.error$.next(undefined)),
			switchMap(([ctx, octokit, reload]) => {
				if (!ctx || !octokit) return EMPTY;
				const prs = loadPrs(ctx, octokit, !!reload?.skipCache);
				this.fresh$.next();
				this.active$.next(true);
				return prs;
			}),
			shareReplay(1),
			catchError((err) => {
				console.log(err);
				this.error$.next(err);
				return of([]);
			})
		);
	}

	async reload(): Promise<void> {
		if (!this.active$.getValue()) return;
		const fresh = firstValueFrom(this.fresh$);
		this.reload$.next({ skipCache: true });
		return await fresh;
	}

	get(branch: string | undefined): Observable<PullRequest | undefined> | undefined {
		if (!branch) return;
		return this.prs$.pipe(map((prs) => prs.find((pr) => pr.targetBranch == branch)));
	}

	/* TODO: Figure out a way to cleanup old behavior subjects */
	getState(branchId: string) {
		let state$ = this.stateMap.get(branchId);
		if (!state$) {
			state$ = new BehaviorSubject<PrServiceState>({ busy: false, branchId });
			this.stateMap.set(branchId, state$);
		}
		return state$;
	}

	private setBusy(action: PrAction, branchId: string) {
		const state$ = this.getState(branchId);
		state$.next({ busy: true, action, branchId });
	}

	private setIdle(branchId: string) {
		const state$ = this.getState(branchId);
		state$.next({ busy: false, branchId });
	}

	async createPullRequest(
		ctx: GitHubIntegrationContext,
		base: string,
		title: string,
		body: string,
		branchId: string
	): Promise<PullRequest | undefined> {
		this.setBusy('creating_pr', branchId);
		const octokit = newClient(ctx);
		try {
			// Wait for branch to have upstream data
			await this.branchController.pushBranch(branchId, true);
			const branch = await firstValueFrom(
				this.branchService.branches$.pipe(
					timeout(10000),
					map((branches) => branches.find((b) => b.id == branchId && b.upstream))
				)
			);
			if (branch?.upstreamName) {
				const rsp = await octokit.rest.pulls.create({
					owner: ctx.owner,
					repo: ctx.repo,
					head: branch.upstreamName,
					base,
					title,
					body
				});
				await this.reload();
				return ghResponseToInstance(rsp.data);
			}
			throw `No upstream for branch ${branchId}`;
		} finally {
			this.setIdle(branchId);
		}
	}

	getStatus(ref: string | undefined): Observable<PrStatus | undefined> | undefined {
		if (!ref) return;
		return combineLatest([this.octokit$, this.ctx$]).pipe(
			switchMap(async ([octokit, ctx]) => {
				if (!octokit || !ctx) return;
				return await octokit.checks.listForRef({
					owner: ctx.owner,
					repo: ctx.repo,
					ref: ref,
					headers: {
						'X-GitHub-Api-Version': '2022-11-28'
					}
				});
			}),
			map((resp) => {
				if (!resp) return;
				const checks = resp?.data.check_runs;
				if (!checks) return;

				const skipped = checks.filter((c) => c.conclusion == 'skipped');
				const succeeded = checks.filter((c) => c.conclusion == 'success');
				const failed = checks.filter((c) => c.conclusion == 'failure');
				const completed = checks.every((check) => !!check.completed_at);

				const count = resp?.data.total_count;
				return {
					success: skipped.length + succeeded.length == count,
					hasChecks: !!count,
					completed,
					failed,
					skipped
				};
			})
		);
	}
}

function loadPrs(
	ctx: GitHubIntegrationContext,
	octokit: Octokit,
	skipCache: boolean
): Observable<PullRequest[]> {
	return new Observable<PullRequest[]>((subscriber) => {
		const key = ctx.owner + '/' + ctx.repo;

		if (!skipCache) {
			const cachedRsp = lscache.get(key);
			if (cachedRsp) subscriber.next(cachedRsp.data.map(ghResponseToInstance));
		}

		try {
			octokit.rest.pulls
				.list({
					owner: ctx.owner,
					repo: ctx.repo
				})
				.then((rsp) => {
					lscache.set(key, rsp, 1440); // 1 day ttl
					subscriber.next(rsp.data.map(ghResponseToInstance));
				});
		} catch (e) {
			console.error(e);
		}
	});
}
