import { parseRemoteUrl } from '$lib/url/gitUrl';
import type { GhResponse } from '$lib/forge/github/ghQuery';
import type {
	DetailedPullRequest,
	Label,
	PullRequest,
	PullRequestPermissions
} from '$lib/forge/interface/types';
import type { RestEndpointMethodTypes } from '@octokit/rest';

export type DetailedGitHubPullRequest = RestEndpointMethodTypes['pulls']['get']['response']['data'];
export type CreatePrResult = RestEndpointMethodTypes['pulls']['create']['response']['data'];
export type CreateIssueResult = RestEndpointMethodTypes['issues']['create']['response']['data'];

export type PullRequestListItem =
	| CreatePrResult
	| RestEndpointMethodTypes['pulls']['list']['response']['data'][number];

export type ChecksResult = RestEndpointMethodTypes['checks']['listForRef']['response']['data'];
export type RepoResult = RestEndpointMethodTypes['repos']['get']['response']['data'];

export interface GitHubRepoPermissions {
	admin: boolean;
	maintain?: boolean;
	push: boolean;
	triage?: boolean;
	pull: boolean;
}

export type DetailedGitHubPullRequestWithPermissions = DetailedGitHubPullRequest & {
	permissions?: GitHubRepoPermissions;
};

export function parseGitHubDetailedPullRequest(
	response: GhResponse<DetailedGitHubPullRequestWithPermissions>
): GhResponse<DetailedPullRequest> {
	if (response.error) {
		return response;
	}
	const data = response.data;

	const reviewers =
		data.requested_reviewers?.map((reviewer) => ({
			srcUrl: reviewer.avatar_url,
			name: reviewer.name || reviewer.login
		})) || [];

	const permissions: PullRequestPermissions | undefined = data.permissions
		? { canMerge: data.permissions.push }
		: undefined;

	return {
		data: {
			id: data.id,
			number: data.number,
			author: data.user
				? {
						name: data.user.login || undefined,
						email: data.user.email || undefined,
						isBot: data.user.type.toLowerCase() === 'bot',
						gravatarUrl: data.user.avatar_url
					}
				: null,
			title: data.title,
			body: data.body ?? undefined,
			baseRepo: parseRemoteUrl(data.base?.repo.git_url),
			baseBranch: data.base?.ref,
			sourceBranch: data.head?.ref,
			draft: data.draft,
			htmlUrl: data.html_url,
			createdAt: data.created_at,
			mergedAt: data.merged_at || undefined,
			closedAt: data.closed_at || undefined,
			updatedAt: data.updated_at,
			merged: data.merged,
			mergeable: !!data.mergeable,
			mergeableState: data.mergeable_state,
			rebaseable: !!data.rebaseable,
			squashable: !!data.mergeable, // Enabled whenever merge is enabled
			state: data.state,
			fork: data.head?.repo?.fork ?? false,
			reviewers,
			commentsCount: data.comments,
			permissions,
			repositorySshUrl: data.head?.repo?.ssh_url,
			repositoryHttpsUrl: data.head?.repo?.clone_url
		}
	};
}

export function ghResponseToInstance(pr: PullRequestListItem): PullRequest {
	const labels: Label[] = pr.labels?.map((label) => ({
		name: label.name,
		description: label.description || undefined,
		color: label.color
	}));

	const reviewers =
		pr.requested_reviewers?.map((reviewer) => ({
			id: reviewer.id,
			srcUrl: reviewer.avatar_url,
			name: reviewer.name || reviewer.login
		})) || [];

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
		createdAt: pr.created_at,
		modifiedAt: pr.created_at,
		sourceBranch: pr.head?.ref,
		targetBranch: pr.base?.ref,
		sha: pr.head?.sha,
		mergedAt: pr.merged_at || undefined,
		closedAt: pr.closed_at || undefined,
		repoOwner: pr.head?.repo?.owner.login,
		repositorySshUrl: pr.head?.repo?.ssh_url,
		repositoryHttpsUrl: pr.head?.repo?.clone_url,
		reviewers
	};
}
