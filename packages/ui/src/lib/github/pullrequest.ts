import { asyncWritable, type WritableLoadable, type Loadable } from '@square/svelte-store';
import lscache from 'lscache';

import { PullRequest, type GitHubIntegrationContext } from '$lib/github/types';
import { newClient } from '$lib/github/client';

// Uses the cached value as the initial state and also in the event of being offline
export function listPullRequestsWithCache(ctx: GitHubIntegrationContext): Loadable<PullRequest[]> {
	const key = ctx.owner + '/' + ctx.repo;
	const store = asyncWritable(
		[],
		async () => lscache.get(key) || [],
		async (data) => data,
		{ trackState: true },
		(set) => {
			listPullRequests(ctx).then((prs) => {
				if (prs !== undefined) {
					lscache.set(key, prs, 1440); // 1 day ttl
					set(prs);
				}
			});
		}
	) as WritableLoadable<PullRequest[]>;
	return store;
}

async function listPullRequests(ctx: GitHubIntegrationContext): Promise<PullRequest[] | undefined> {
	const octokit = newClient(ctx);
	try {
		const rsp = await octokit.rest.pulls.list({
			owner: ctx.owner,
			repo: ctx.repo
		});
		return rsp.data.map(PullRequest.fromApi);
	} catch (e) {
		console.log(e);
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
