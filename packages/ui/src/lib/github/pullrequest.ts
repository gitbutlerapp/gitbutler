import { Octokit } from '@octokit/rest';
import { PullRequest, User, Label, type GitHubIntegrationContext } from '$lib/github/types';
import type { RestEndpointMethodTypes } from '@octokit/rest';
import { asyncWritable, type WritableLoadable, type Loadable } from '@square/svelte-store';
import lscache from 'lscache';

function newClient(ctx: GitHubIntegrationContext) {
	return new Octokit({
		auth: ctx.authToken,
		userAgent: 'GitButler Client',
		baseUrl: 'https://api.github.com'
	});
}

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
		return rsp.data.map(fromApiPullRequest);
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
			return fromApiPullRequest(pr);
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
		return fromApiPullRequest(pr);
	} catch (e) {
		console.log(e);
	}
}

function fromApiPullRequest(
	pr:
		| RestEndpointMethodTypes['pulls']['create']['response']['data']
		| RestEndpointMethodTypes['pulls']['list']['response']['data'][number]
): PullRequest {
	const author = pr.user
		? new User(pr.user.login, pr.user.email || undefined, pr.user.type === 'Bot')
		: undefined;
	const labels = pr.labels.map((label) => {
		return new Label(label.name, label.description || undefined, label.color);
	});

	return new PullRequest(
		pr.html_url,
		pr.number,
		pr.title,
		pr.body || undefined,
		author,
		labels,
		pr.draft || false,
		pr.created_at,
		pr.head.ref,
		pr.base.ref
	);
}
