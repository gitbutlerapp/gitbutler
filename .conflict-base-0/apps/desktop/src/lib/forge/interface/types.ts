import type { Author } from '$lib/commits/commit';
import type { RepoInfo } from '$lib/url/gitUrl';

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
	createdAt: string;
	modifiedAt: string;
	mergedAt?: string;
	closedAt?: string;
	repositorySshUrl?: string;
	repositoryHttpsUrl?: string;
	repoOwner?: string;
	reviewers: { srcUrl: string; name: string }[];
}

export interface DetailedPullRequest {
	id: number;
	title: string;
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
	reviewers: { srcUrl: string; name: string }[];
	commentsCount: number;
}

export type ChecksStatus = {
	startedAt: string;
	completed: boolean;
	success: boolean;
};

export enum MergeMethod {
	Merge = 'merge',
	Rebase = 'rebase',
	Squash = 'squash'
}
export type CheckSuites = {
	count: number;
	items?: CheckSuite[];
};

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
