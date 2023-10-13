import { Octokit } from '@octokit/rest';
import type { GitHubIntegrationContext, PullRequest } from '$lib/github/types';

function newClient(ctx: GitHubIntegrationContext) {
	return new Octokit({
		auth: ctx.authToken,
		userAgent: 'GitButler Client',
		baseUrl: 'https://api.github.com'
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
		const item = rsp.data.find((pr) => pr !== undefined)?.html_url;
		if (item) {
			return { html_url: item };
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
		return { html_url: rsp.data.html_url };
	} catch (e) {
		console.log(e);
	}
}
