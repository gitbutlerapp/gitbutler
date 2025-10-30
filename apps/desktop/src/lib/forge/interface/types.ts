import type { Author } from '$lib/commits/commit';
import type { RepoInfo } from '$lib/url/gitUrl';

export interface Label {
	name: string;
	description: string | undefined;
	color: string;
}

export type ForgeUser = {
	id: number;
	srcUrl: string;
	name: string;
};

export type ForgeUserDetailed = {
	id: number;
	login: string;
	name: string | null;
	email: string | null;
	avatarUrl: string | null;
	isBot: boolean;
};

export type ForgeReview = {
	htmlUrl: string;
	number: number;
	title: string;
	body: string | null;
	author: ForgeUserDetailed | null;
	labels: Label[];
	draft: boolean;
	sourceBranch: string;
	targetBranch: string;
	sha: string;
	createdAt: string | null;
	modifiedAt: string | null;
	mergedAt: string | null;
	closedAt: string | null;
	repositorySshUrl: string | null;
	repositoryHttpsUrl: string | null;
	repoOwner: string | null;
	reviewers: ForgeUserDetailed[];
};

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
	createdAt: string;
	modifiedAt: string;
	mergedAt?: string;
	closedAt?: string;
	repositorySshUrl?: string;
	repositoryHttpsUrl?: string;
	repoOwner?: string;
	reviewers: ForgeUser[];
}

function getCleanBranchName(sourceBranch: string): string {
	if (sourceBranch.includes(':')) {
		const parts = sourceBranch.split(':');
		return parts[parts.length - 1] ?? '';
	}
	return sourceBranch;
}

export function mapForgeReviewToPullRequest(pr: ForgeReview): PullRequest {
	return {
		htmlUrl: pr.htmlUrl,
		number: pr.number,
		title: pr.title,
		body: pr.body ?? undefined,
		author: pr.author
			? {
					name: pr.author.name ?? pr.author.login,
					email: pr.author.email ?? undefined,
					gravatarUrl: pr.author.avatarUrl ?? undefined,
					isBot: pr.author.isBot
				}
			: null,
		labels: pr.labels,
		draft: pr.draft,
		sourceBranch: getCleanBranchName(pr.sourceBranch),
		targetBranch: pr.targetBranch,
		sha: pr.sha,
		createdAt: pr.createdAt ?? '',
		modifiedAt: pr.modifiedAt ?? '',
		mergedAt: pr.mergedAt ?? undefined,
		closedAt: pr.closedAt ?? undefined,
		repositorySshUrl: pr.repositorySshUrl ?? undefined,
		repositoryHttpsUrl: pr.repositoryHttpsUrl ?? undefined,
		repoOwner: pr.repoOwner ?? undefined,
		reviewers: pr.reviewers.map((r) => ({
			id: r.id,
			srcUrl: r.avatarUrl ?? '',
			name: r.name ?? r.login
		}))
	};
}

export interface PullRequestPermissions {
	canMerge?: boolean;
}

export interface DetailedPullRequest {
	id: number;
	title: string;
	author: Author | null;
	body: string | undefined;
	number: number;
	sourceBranch: string;
	draft?: boolean;
	fork: boolean;
	createdAt: string;
	mergedAt?: string;
	closedAt?: string;
	updatedAt: string;
	htmlUrl: string;
	merged: boolean;
	mergeable: boolean;
	mergeableState: string;
	rebaseable: boolean;
	squashable: boolean;
	state: 'open' | 'closed';
	baseRepo?: RepoInfo | undefined;
	baseBranch: string;
	reviewers: { srcUrl: string; username: string }[];
	commentsCount: number;
	permissions?: PullRequestPermissions;
	repositorySshUrl?: string;
	repositoryHttpsUrl?: string;
}

export type ChecksStatus = {
	startedAt: string;
	/**
	 * Checks are considered completed if all checks have completed  or if there is at least one failure.
	 */
	completed: boolean;
	success: boolean;
	failedChecks: string[];
};

export enum MergeMethod {
	Merge = 'merge',
	Rebase = 'rebase',
	Squash = 'squash'
}

export type CheckSuite = {
	name?: string;
	count: number;
	status: 'queued' | 'in_progress' | 'completed' | 'waiting' | 'requested' | 'pending' | null;
};

export type ForgeArguments = {
	repo: RepoInfo;
	baseBranch: string;
	forkStr?: string;
	authenticated: boolean;
};

export type CreatePullRequestArgs = {
	title: string;
	body: string;
	draft: boolean;
	baseBranchName: string;
	upstreamName: string;
};
