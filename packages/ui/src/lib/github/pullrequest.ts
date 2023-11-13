import type { Loadable } from '@square/svelte-store';
import lscache from 'lscache';
import { Observable, EMPTY, type UnaryFunction, type OperatorFunction, pipe } from 'rxjs';
import { filter, shareReplay, skipWhile, switchMap } from 'rxjs/operators';
import { storeToObservable } from '$lib/rxjs/store';

import { PullRequest, type GitHubIntegrationContext } from '$lib/github/types';
import { newClient } from '$lib/github/client';

function filterNullish<T>(): UnaryFunction<Observable<T | null | undefined>, Observable<T>> {
	return pipe(filter((x) => x != null) as OperatorFunction<T | null | undefined, T>);
}

// Uses the cached value as the initial state and also in the event of being offline
export function listPullRequestsWithCache(
	ghContextStore: Loadable<GitHubIntegrationContext | undefined>
): Observable<PullRequest[]> {
	const ghContext$ = storeToObservable(ghContextStore);
	return ghContext$.pipe(
		filterNullish(),
		switchMap((ctx) => {
			return new Observable<PullRequest[]>((observer) => {
				const key = ctx.owner + '/' + ctx.repo;
				const cachedPrs = lscache.get(key);
				if (cachedPrs) {
					observer.next(cachedPrs);
				}
				listPullRequests(ctx).then((prs) => {
					observer.next(prs);
					lscache.set(key, prs, 1440); // 1 day ttl
				});
			});
		}),
		shareReplay(1)
	);
}

async function listPullRequests(ctx: GitHubIntegrationContext): Promise<PullRequest[]> {
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
