import type { RepoInfo } from '$lib/url/gitUrl';
import type { Author } from '$lib/vbranches/types';
import type { GitHubPullRequestId } from '../github/types';

export interface Label {
	name: string;
	description: string | undefined;
	color: string;
}

export interface PullRequest {
	id: PullRequestId;
	htmlUrl: string;
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

export interface DetailedPullRequest {
	id: PullRequestId;
	title: string;
	body: string | undefined;
	sourceBranch: string;
	draft?: boolean;
	createdAt: Date;
	mergedAt?: Date;
	closedAt?: Date;
	htmlUrl: string;
	mergeable: boolean;
	mergeableState: string;
	rebaseable: boolean;
	squashable: boolean;
	state: 'open' | 'closed';
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

export enum ForgeName {
	GitHub = 'GitHub',
	GitLab = 'GitLab',
	BitBucket = 'BitBucket',
	Azure = 'Azure'
}

/**
 * Represents an identifier for the series at possible forges, e.g. a GitHub PR number.
 */
export type PullRequestId =
	| { type: ForgeName.GitHub; subject: GitHubPullRequestId }
	| { type: ForgeName.GitLab; subject: void }
	| { type: ForgeName.Azure; subject: void }
	| { type: ForgeName.BitBucket; subject: void };
