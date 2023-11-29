import lscache from 'lscache';
import { Observable, EMPTY, BehaviorSubject, of, firstValueFrom, Subject } from 'rxjs';
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
	ghResponseToInstance
} from '$lib/github/types';
import { newClient } from '$lib/github/client';
import type { BranchController } from '$lib/vbranches/branchController';
import type { VirtualBranchService } from '$lib/vbranches/branchStoresCache';

export type PrAction = 'creating_pr';
export type PrServiceState = { busy: boolean; branchId: string; action?: PrAction };

export class PrService {
	prs$: Observable<PullRequest[]>;
	error$ = new BehaviorSubject<string | undefined>(undefined);

	private stateMap = new Map<string, BehaviorSubject<PrServiceState>>();
	private reload$ = new BehaviorSubject<{ skipCache: boolean } | undefined>(undefined);
	private fresh$ = new Subject<void>();

	constructor(
		private branchController: BranchController,
		private branchService: VirtualBranchService,
		ghContext$: Observable<GitHubIntegrationContext | undefined>
	) {
		this.prs$ = ghContext$.pipe(
			combineLatestWith(this.reload$),
			tap(() => this.error$.next(undefined)),
			switchMap(([ctx, reload]) => {
				if (!ctx) return EMPTY;
				const prs = loadPrs(ctx, !!reload?.skipCache);
				this.fresh$.next();
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

	reload(): Promise<void> {
		this.reload$.next({ skipCache: true });
		return firstValueFrom(this.fresh$);
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
				return ghResponseToInstance(rsp.data);
			}
			throw `No upstream for branch ${branchId}`;
		} finally {
			this.setIdle(branchId);
		}
	}
}

function loadPrs(ctx: GitHubIntegrationContext, skipCache: boolean): Observable<PullRequest[]> {
	return new Observable<PullRequest[]>((subscriber) => {
		const key = ctx.owner + '/' + ctx.repo;

		if (!skipCache) {
			const cachedRsp = lscache.get(key);
			if (cachedRsp) subscriber.next(cachedRsp.data.map(ghResponseToInstance));
		}

		const octokit = newClient(ctx);
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

export async function getPullRequestByBranch(
	ctx: GitHubIntegrationContext,
	branch: string
): Promise<PullRequest | undefined> {
	const octokit = newClient(ctx);
	try {
		const rsp = await octokit.rest.pulls.list({
			owner: ctx.owner,
			repo: ctx.repo,
			head: ctx.owner + ':' + branch
		});
		// at most one pull request per head / branch
		const pr = rsp.data.find((pr) => pr !== undefined);
		if (pr) {
			return ghResponseToInstance(pr);
		}
	} catch (e) {
		console.log(e);
	}
}
