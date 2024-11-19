import type { RepoInfo } from '$lib/url/gitUrl';
import type { Author } from '$lib/vbranches/types';

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
	repositorySshUrl?: string;
	repositoryHttpsUrl?: string;
	repoOwner?: string;
}

export interface DetailedPullRequest {
	id: number;
	title: string;
	body: string | undefined;
	number: number;
	sourceBranch: string;
	draft?: boolean;
	fork?: boolean;
	createdAt: Date;
	mergedAt?: Date;
	closedAt?: Date;
	htmlUrl: string;
	merged: boolean;
	mergeable: boolean;
	mergeableState: string;
	rebaseable: boolean;
	squashable: boolean;
	state: 'open' | 'closed';
	baseRepo: RepoInfo | undefined;
	baseBranch: string;
}

export type ChecksStatus = {
	startedAt: Date;
	completed: boolean;
	success: boolean;
	hasChecks: boolean;
	failed: number;
	queued: number;
	totalCount: number;
	skipped: number;
	finished: number;
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
};

export type CreatePullRequestArgs = {
	title: string;
	body: string;
	draft: boolean;
	baseBranchName: string;
	upstreamName: string;
};
