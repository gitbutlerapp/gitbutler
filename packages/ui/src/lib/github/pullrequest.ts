import lscache from 'lscache';
import { Observable, EMPTY, BehaviorSubject, of, firstValueFrom, Subject } from 'rxjs';
import {
	catchError,
	combineLatestWith,
	filter,
	map,
	shareReplay,
	switchMap,
	tap
} from 'rxjs/operators';

import {
	type PullRequest,
	type GitHubIntegrationContext,
	ghResponseToInstance
} from '$lib/github/types';
import { newClient } from '$lib/github/client';

export type PrAction = 'creating_pr';
export type PrServiceState = { busy: boolean; action?: PrAction; branchId?: string };

export class PrService {
	prs$: Observable<PullRequest[]>;
	error$ = new BehaviorSubject<string | undefined>(undefined);
	state$ = new BehaviorSubject<PrServiceState>({ busy: false });
	private reload$ = new BehaviorSubject<{ skipCache: boolean } | undefined>(undefined);
	private fresh$ = new Subject<void>();

	constructor(ghContext$: Observable<GitHubIntegrationContext | undefined>) {
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

	getState(branchId: string) {
		return this.state$.pipe(filter((b) => b.branchId == branchId));
	}

	private setBusy(action: PrAction) {
		this.state$.next({ busy: true, action });
	}

	private setIdle() {
		this.state$.next({ busy: false });
	}

	async createPullRequest(
		ctx: GitHubIntegrationContext,
		head: string,
		base: string,
		title: string,
		body: string
	): Promise<PullRequest | undefined> {
		const octokit = newClient(ctx);
		this.setBusy('creating_pr');
		try {
			const rsp = await octokit.rest.pulls.create({
				owner: ctx.owner,
				repo: ctx.repo,
				head,
				base,
				title,
				body
			});
			return ghResponseToInstance(rsp.data);
		} catch (e) {
			console.log(e);
		} finally {
			this.setIdle();
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
