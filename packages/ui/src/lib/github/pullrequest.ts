import lscache from 'lscache';
import { Observable, EMPTY, BehaviorSubject, of } from 'rxjs';
import { catchError, combineLatestWith, map, shareReplay, switchMap, tap } from 'rxjs/operators';

import {
	type PullRequest,
	type GitHubIntegrationContext,
	ghResponseToInstance
} from '$lib/github/types';
import { newClient } from '$lib/github/client';

export class PrService {
	prs$: Observable<PullRequest[]>;
	error$ = new BehaviorSubject<string | undefined>(undefined);
	private reload$ = new BehaviorSubject<{ skipCache: boolean } | undefined>(undefined);
	private inject$ = new BehaviorSubject<PullRequest | undefined>(undefined);

	constructor(ghContext$: Observable<GitHubIntegrationContext | undefined>) {
		this.prs$ = ghContext$.pipe(
			tap((context) => console.log('context', context)),
			combineLatestWith(this.reload$),
			tap(() => this.error$.next(undefined)),
			switchMap(([ctx, reload]) => {
				if (!ctx) return EMPTY;
				console.log('loading prs');
				return loadPrs(ctx, !!reload?.skipCache);
			}),
			combineLatestWith(this.inject$),
			map(([prs, inject]) => {
				console.log('inject', inject, prs);
				if (inject) return prs.concat(inject);
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

	reload(): void {
		console.log('reload prs');
		this.reload$.next({ skipCache: true });
	}
	insert(pr: PullRequest) {
		console.log('insert into cache', pr);
		this.inject$.next(pr);
	}
	get(branch: string | undefined): Observable<PullRequest | undefined> | undefined {
		if (!branch) return;
		console.log('getting pr', branch);
		return this.prs$.pipe(map((prs) => prs.find((pr) => pr.targetBranch == branch)));
	}
}

function loadPrs(ctx: GitHubIntegrationContext, skipCache: boolean): Observable<PullRequest[]> {
	console.log('actually loading prs, skip cache', skipCache);
	return new Observable<PullRequest[]>((subscriber) => {
		const key = ctx.owner + '/' + ctx.repo;

		if (!skipCache) {
			console.log('using cache');
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

export async function createPullRequest(
	ctx: GitHubIntegrationContext,
	head: string,
	base: string,
	title: string,
	body: string
): Promise<PullRequest | undefined> {
	const octokit = newClient(ctx);
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
	}
}
