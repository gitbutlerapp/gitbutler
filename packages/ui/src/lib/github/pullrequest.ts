import lscache from 'lscache';
import { Observable, EMPTY, BehaviorSubject, Subject, of } from 'rxjs';
import { catchError, shareReplay, switchMap, tap, withLatestFrom } from 'rxjs/operators';

import { PullRequest, type GitHubIntegrationContext } from '$lib/github/types';
import { newClient } from '$lib/github/client';

export class PrService {
	prs$: Observable<PullRequest[]>;
	error$ = new BehaviorSubject<string | undefined>(undefined);
	reload$ = new BehaviorSubject<void>(undefined);

	constructor(ghContext$: Observable<GitHubIntegrationContext | undefined>) {
		this.prs$ = ghContext$.pipe(
			withLatestFrom(this.reload$),
			tap(() => this.error$.next(undefined)),
			switchMap(([ctx, _]) => {
				if (!ctx) return EMPTY;
				return loadPrs(ctx);
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
		this.reload$.next();
	}
}

function loadPrs(ctx: GitHubIntegrationContext): Observable<PullRequest[]> {
	// throw 'An ad-hoc error';
	return new Observable<PullRequest[]>((subscriber) => {
		const key = ctx.owner + '/' + ctx.repo;

		const cachedPrs = lscache.get(key);
		if (cachedPrs) subscriber.next(cachedPrs);

		fetchPrs(ctx).then((prs) => {
			subscriber.next(prs);
			lscache.set(key, prs, 1440); // 1 day ttl
		});
	});
}

async function fetchPrs(ctx: GitHubIntegrationContext): Promise<PullRequest[]> {
	const octokit = newClient(ctx);
	try {
		const rsp = await octokit.rest.pulls.list({
			owner: ctx.owner,
			repo: ctx.repo
		});
		return rsp.data.map(PullRequest.fromApi);
	} catch (e) {
		console.error(e);
		return [];
	}
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
			return PullRequest.fromApi(pr);
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
		const pr = rsp.data;
		return PullRequest.fromApi(pr);
	} catch (e) {
		console.log(e);
	}
}
