import {
	ForgeName,
	type CheckSuite,
	type DetailedPullRequest,
	type Label,
	type PullRequest
} from '$lib/forge/interface/types';
import type { RestEndpointMethodTypes } from '@octokit/rest';

type DetailedGitHubPullRequest = RestEndpointMethodTypes['pulls']['get']['response']['data'];

/**
 * Represents a GitHub pull request identifier.
 */
export interface GitHubPullRequestId {
	prNumber: number;
}

export function parseGitHubDetailedPullRequest(
	data: DetailedGitHubPullRequest
): DetailedPullRequest {
	return {
		id: { type: ForgeName.GitHub, subject: { prNumber: data.number } },
		title: data.title,
		body: data.body ?? undefined,
		sourceBranch: data.head?.ref,
		draft: data.draft,
		htmlUrl: data.html_url,
		createdAt: new Date(data.created_at),
		mergedAt: data.merged_at ? new Date(data.merged_at) : undefined,
		closedAt: data.closed_at ? new Date(data.closed_at) : undefined,
		mergeable: !!data.mergeable,
		mergeableState: data.mergeable_state,
		rebaseable: !!data.rebaseable,
		squashable: !!data.mergeable, // Enabled whenever merge is enabled
		state: data.state
	};
}

export function ghResponseToInstance(
	pr:
		| RestEndpointMethodTypes['pulls']['create']['response']['data']
		| RestEndpointMethodTypes['pulls']['list']['response']['data'][number]
): PullRequest {
	const labels: Label[] = pr.labels?.map((label) => ({
		name: label.name,
		description: label.description || undefined,
		color: label.color
	}));

	return {
		id: { type: ForgeName.GitHub, subject: { prNumber: pr.number } },
		htmlUrl: pr.html_url,
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
		sourceBranch: pr.head?.ref,
		targetBranch: pr.base?.ref,
		sha: pr.head?.sha,
		mergedAt: pr.merged_at ? new Date(pr.merged_at) : undefined,
		closedAt: pr.closed_at ? new Date(pr.closed_at) : undefined,
		repoName: pr.head?.repo?.full_name,
		repositorySshUrl: pr.head?.repo?.ssh_url,
		repositoryHttpsUrl: pr.head?.repo?.clone_url
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
