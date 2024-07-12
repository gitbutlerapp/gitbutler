import type { CheckSuite, DetailedPullRequest, Label, PullRequest } from '../interface/types';
import type { RestEndpointMethodTypes } from '@octokit/rest';

export type DetailedGitHubPullRequest = RestEndpointMethodTypes['pulls']['get']['response']['data'];

export function parseGitHubDetailedPullRequest(
	data: DetailedGitHubPullRequest
): DetailedPullRequest {
	return {
		id: data.id,
		number: data.number,
		title: data.title,
		sourceBranch: data.base?.ref,
		draft: data.draft,
		htmlUrl: data.html_url,
		createdAt: new Date(data.created_at),
		mergedAt: data.merged_at ? new Date(data.merged_at) : undefined,
		closedAt: data.closed_at ? new Date(data.closed_at) : undefined,
		mergeable: !!data.mergeable,
		mergeableState: data.mergeable_state,
		rebaseable: !!data.rebaseable,
		squashable: !!data.mergeable // Enabled whenever merge is enabled
	};
}

export function ghResponseToInstance(
	pr:
		| RestEndpointMethodTypes['pulls']['create']['response']['data']
		| RestEndpointMethodTypes['pulls']['list']['response']['data'][number]
): PullRequest {
	const labels: Label[] = pr.labels.map((label) => ({
		name: label.name,
		description: label.description || undefined,
		color: label.color
	}));

	return {
		htmlUrl: pr.html_url,
		number: pr.number,
		title: pr.title,
		body: pr.body || undefined,
		author: pr.user
			? {
					name: pr.user.login || undefined,
					email: pr.user.email || undefined,
					isBot: pr.user.type.toLowerCase() === 'bot',
					gravatarUrl: pr.user.avatar_url
				}
			: null,
		labels: labels,
		draft: pr.draft || false,
		createdAt: new Date(pr.created_at),
		modifiedAt: new Date(pr.created_at),
		sourceBranch: pr.head.ref,
		targetBranch: pr.base.ref,
		sha: pr.head.sha,
		mergedAt: pr.merged_at ? new Date(pr.merged_at) : undefined,
		closedAt: pr.closed_at ? new Date(pr.closed_at) : undefined,
		repoName: pr.head.repo?.full_name,
		repositorySshUrl: pr.head.repo?.ssh_url,
		repositoryHttpsUrl: pr.head.repo?.clone_url
	};
}

export type GitHubListCheckSuitesResp =
	RestEndpointMethodTypes['checks']['listSuitesForRef']['response']['data'];
export type GitHubCheckSuites =
	RestEndpointMethodTypes['checks']['listSuitesForRef']['response']['data']['check_suites'];

export function parseGitHubCheckSuites(data: GitHubListCheckSuitesResp): CheckSuite[] {
	const result = data.check_suites.map((checkSuite) => ({
		name: checkSuite.app?.name,
		status: checkSuite.status,
		count: checkSuite.latest_check_runs_count
	}));
	return result;
}
