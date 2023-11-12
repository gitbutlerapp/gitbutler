import type { Loadable } from '@square/svelte-store';
import lscache from 'lscache';
import { Observable, EMPTY } from 'rxjs';
import { switchMap } from 'rxjs/operators';
import { storeToObservable } from '$lib/rxjs/store';

import { PullRequest, type GitHubIntegrationContext } from '$lib/github/types';
import { newClient } from '$lib/github/client';

// Uses the cached value as the initial state and also in the event of being offline
export function listPullRequestsWithCache(
	ghContextStore: Loadable<GitHubIntegrationContext | undefined>
): Observable<PullRequest[]> {
	const ghContextObservable = storeToObservable(ghContextStore);
	const prsObservable = ghContextObservable.pipe(
		switchMap((ctx) => {
			if (!ctx) return EMPTY;
			const obs: Observable<PullRequest[]> = new Observable((observer) => {
				const key = ctx.owner + '/' + ctx.repo;
				const cachedPrs = lscache.get(key);
				if (cachedPrs) {
					observer.next(cachedPrs);
				}
				const request = listPullRequests(ctx);
				request.then((prs) => {
					if (prs) {
						observer.next(prs);
						lscache.set(key, prs, 1440); // 1 day ttl
					}
				});
			});
			return obs;
		})
	);
	return prsObservable;
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
