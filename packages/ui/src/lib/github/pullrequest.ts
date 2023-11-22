import lscache from 'lscache';
import { Observable, EMPTY, BehaviorSubject, of } from 'rxjs';
import { catchError, combineLatestWith, shareReplay, switchMap, tap } from 'rxjs/operators';

import {
	type PullRequest,
	type GitHubIntegrationContext,
	ghResponseToInstance
} from '$lib/github/types';
import { newClient } from '$lib/github/client';

export class PrService {
	prs$: Observable<PullRequest[]>;
	error$ = new BehaviorSubject<string | undefined>(undefined);
	private reload$ = new BehaviorSubject<void>(undefined);

	constructor(ghContext$: Observable<GitHubIntegrationContext | undefined>) {
		this.prs$ = ghContext$.pipe(
			combineLatestWith(this.reload$),
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
	return new Observable<PullRequest[]>((subscriber) => {
		const key = ctx.owner + '/' + ctx.repo;

		const cachedRsp = lscache.get(key);
		if (cachedRsp) subscriber.next(cachedRsp.data.map(ghResponseToInstance));

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
