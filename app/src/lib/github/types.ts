import type { Author } from '$lib/vbranches/types';
import type { RestEndpointMethodTypes } from '@octokit/rest';

export interface Label {
	name: string;
	description: string | undefined;
	color: string;
}

export interface PullRequest {
	htmlUrl: string;
	number: number;
	title: string;
	body: string | undefined;
	author: Author | null;
	labels: Label[];
	draft: boolean;
	sourceBranch: string;
	targetBranch: string;
	sha: string;
	createdAt: Date;
	modifiedAt: Date;
	mergedAt?: Date;
	closedAt?: Date;
	repoName?: string;
	repositorySshUrl?: string;
	repositoryHttpsUrl?: string;
}

export type DetailedGitHubPullRequest = RestEndpointMethodTypes['pulls']['get']['response']['data'];

export interface DetailedPullRequest {
	targetBranch: string;
	draft?: boolean;
	createdAt: Date;
	mergedAt?: Date;
	closedAt?: Date;
	htmlUrl: string;
	mergeable: boolean;
	mergeableState: string;
	rebaseable: boolean;
	squashable: boolean;
}

export function parseGitHubDetailedPullRequest(
	data: DetailedGitHubPullRequest
): DetailedPullRequest {
	return {
		targetBranch: data.base.ref,
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

export type ChecksStatus =
	| {
			startedAt: Date;
			completed: boolean;
			success: boolean;
			hasChecks: boolean;
			failed: number;
			queued: number;
			totalCount: number;
			skipped: number;
			finished: number;
	  }
	| null
	| undefined;

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
					gravatarUrl: new URL(pr.user.avatar_url)
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

export enum MergeMethod {
	Merge = 'merge',
	Rebase = 'rebase',
	Squash = 'squash'
}

export type GitHubListCheckSuitesResp =
	RestEndpointMethodTypes['checks']['listSuitesForRef']['response']['data'];
export type GitHubCheckSuites =
	RestEndpointMethodTypes['checks']['listSuitesForRef']['response']['data']['check_suites'];

export type CheckSuites =
	| {
			count: number;
			items?: CheckSuite[];
	  }
	| null
	| undefined;

export type CheckSuite = {
	name?: string;
	count: number;
	status: 'queued' | 'in_progress' | 'completed' | 'waiting' | 'requested' | 'pending' | null;
};

export function parseGitHubCheckSuites(data: GitHubListCheckSuitesResp): CheckSuite[] {
	const result = data.check_suites.map((checkSuite) => ({
		name: checkSuite.app?.name,
		status: checkSuite.status,
		count: checkSuite.latest_check_runs_count
	}));
	return result;
}
