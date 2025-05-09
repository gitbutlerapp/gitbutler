import type { Author, Commit, CommitState, UpstreamCommit } from '$lib/branches/v3';
import type { HunkHeader } from '$lib/hunks/hunk';
import type { BranchDetails, Stack, StackDetails } from '$lib/stacks/stack';

export const MOCK_STACK_A_ID = '1234-123';

export const MOCK_STACK_A: Stack = {
	id: MOCK_STACK_A_ID,
	heads: [
		{
			name: 'branch-a',
			tip: '1234123'
		}
	],
	tip: '1234123'
};

export const MOCK_BRAND_NEW_BRANCH_NAME = 'super-cool-branch-name';

export const MOCK_STACK_BRAND_NEW_ID = 'empty-stack';

export const MOCK_STACK_BRAND_NEW: Stack = {
	id: MOCK_STACK_BRAND_NEW_ID,
	heads: [
		{
			name: MOCK_BRAND_NEW_BRANCH_NAME,
			tip: '1234123'
		}
	],
	tip: '1234123'
};

export const MOCK_STACKS: Stack[] = [MOCK_STACK_A];

export const MOCK_AUTHOR: Author = {
	name: 'Test Author',
	email: 'author@example.com',
	gravatarUrl: 'https://avatars.githubusercontent.com/u/35891811?v=4'
};

export const MOCK_COMMIT_STATE_LOCAL: CommitState = { type: 'LocalOnly' };
export const MOCK_COMMIT_STATE_INTEGRATED: CommitState = { type: 'Integrated' };
export const MOCK_COMMIT_STATE_LOCAL_AND_REMOTE_DIVERGED: CommitState = {
	type: 'LocalAndRemote',
	subject: 'remote-commit'
};

export const MOCK_COMMIT: Commit = {
	id: '1234123',
	parentIds: ['parent-sha'],
	message: 'Initial commit',
	hasConflicts: false,
	state: MOCK_COMMIT_STATE_LOCAL,
	createdAt: 1714000000000,
	author: MOCK_AUTHOR
};

export function createCommit(override: Partial<Commit>): Commit {
	return {
		...MOCK_COMMIT,
		...override
	};
}

export const MOCK_UPSTREAM_COMMIT: UpstreamCommit = {
	id: 'upstream-sha',
	message: 'Upstream commit',
	createdAt: 1714000000001,
	author: MOCK_AUTHOR
};

export function createMockUpstreamCommit(override: Partial<UpstreamCommit>): UpstreamCommit {
	return {
		...MOCK_UPSTREAM_COMMIT,
		...override
	};
}

export const MOCK_BRANCH_DETAILS: BranchDetails = {
	name: 'branch-a',
	remoteTrackingBranch: null,
	description: 'A mock branch for testing',
	prNumber: null,
	reviewId: null,
	tip: '1234123',
	baseCommit: 'base-sha',
	pushStatus: 'completelyUnpushed',
	lastUpdatedAt: Date.now(),
	authors: [MOCK_AUTHOR],
	isConflicted: false,
	commits: [MOCK_COMMIT],
	upstreamCommits: [],
	isRemoteHead: false
};

export const MOCK_BRANCH_DETAILS_BRAND_NEW: BranchDetails = {
	name: MOCK_BRAND_NEW_BRANCH_NAME,
	remoteTrackingBranch: null,
	description: 'A mock branch for testing',
	prNumber: null,
	reviewId: null,
	tip: '1234123',
	baseCommit: 'base-sha',
	pushStatus: 'completelyUnpushed',
	lastUpdatedAt: Date.now(),
	authors: [],
	isConflicted: false,
	commits: [],
	upstreamCommits: [],
	isRemoteHead: false
};

export function createMockBranchDetails(overrides: Partial<BranchDetails> = {}): BranchDetails {
	return {
		...MOCK_BRANCH_DETAILS,
		...overrides
	};
}

export const MOCK_STACK_DETAILS_BRAND_NEW: StackDetails = {
	derivedName: MOCK_BRAND_NEW_BRANCH_NAME,
	pushStatus: 'completelyUnpushed',
	branchDetails: [MOCK_BRANCH_DETAILS_BRAND_NEW],
	isConflicted: false
};

export const MOCK_STACK_DETAILS: StackDetails = {
	derivedName: 'branch-a',
	pushStatus: 'completelyUnpushed',
	branchDetails: [MOCK_BRANCH_DETAILS],
	isConflicted: false
};

export function createMockStackDetails(overrides: Partial<StackDetails> = {}): StackDetails {
	return {
		...MOCK_STACK_DETAILS,
		...overrides
	};
}

export type UpdateCommitMessageParams = {
	projectId: string;
	stackId: string;
	commitOid: string;
	message: string;
};

export function isUpdateCommitMessageParams(params: unknown): params is UpdateCommitMessageParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		typeof params.projectId === 'string' &&
		'stackId' in params &&
		typeof params.stackId === 'string' &&
		'commitOid' in params &&
		typeof params.commitOid === 'string' &&
		'message' in params &&
		typeof params.message === 'string'
	);
}

export type StackDetailsParams = {
	projectId: string;
	stackId: string;
};

export function isStackDetailsParams(params: unknown): params is StackDetailsParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		typeof params.projectId === 'string' &&
		'stackId' in params &&
		typeof params.stackId === 'string'
	);
}
export type CreateCommitParamsWorktreeChanges = {
	previousPathBytes?: number[];
	pathBytes: number[];
	hunkHeaders: HunkHeader[];
};

export type CreateCommitParams = {
	stackId: string;
	message: string;
	/** Undefined means that the backend will infer the parent to be the current head of stackBranchName */
	parentId: string | undefined;
	stackBranchName: string;
	worktreeChanges: CreateCommitParamsWorktreeChanges[];
};

export function isHunkHeader(something: unknown): something is HunkHeader {
	return (
		typeof something === 'object' &&
		something !== null &&
		'oldStart' in something &&
		typeof something['oldStart'] === 'number' &&
		'oldLines' in something &&
		typeof something['oldLines'] === 'number' &&
		'newStart' in something &&
		typeof something['newStart'] === 'number' &&
		'newLines' in something &&
		typeof something['newLines'] === 'number'
	);
}

export function isCreateCommitRequestWorktreeChanges(
	something: unknown
): something is CreateCommitParamsWorktreeChanges {
	return (
		typeof something === 'object' &&
		something !== null &&
		((Array.isArray((something as any).previousPathBytes) &&
			(something as any).previousPathBytes.every((byte: any) => typeof byte === 'number')) ||
			(something as any)['previousPathBytes'] === null) &&
		'pathBytes' in something &&
		Array.isArray(something['pathBytes']) &&
		something['pathBytes'].every((byte) => typeof byte === 'number') &&
		'hunkHeaders' in something &&
		Array.isArray(something['hunkHeaders']) &&
		something['hunkHeaders'].every((header) => isHunkHeader(header))
	);
}

export function isCreateCommitParams(args: unknown): args is CreateCommitParams {
	return (
		typeof args === 'object' &&
		args !== null &&
		'stackId' in args &&
		typeof args['stackId'] === 'string' &&
		'message' in args &&
		typeof args['message'] === 'string' &&
		'parentId' in args &&
		(typeof args['parentId'] === 'string' || args['parentId'] === undefined) &&
		'stackBranchName' in args &&
		typeof args['stackBranchName'] === 'string' &&
		'worktreeChanges' in args &&
		Array.isArray(args['worktreeChanges']) &&
		args['worktreeChanges'].every((change) => isCreateCommitRequestWorktreeChanges(change))
	);
}
