import type { Author, Commit, CommitState, UpstreamCommit } from '$lib/branches/v3';
import type { BranchDetails, Stack, StackDetails } from '$lib/stacks/stack';

export const MOCK_STACK_A: Stack = {
	id: '1234123',
	heads: [
		{
			name: 'branch-a',
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

export const MOCK_COMMIT_STATE: CommitState = { type: 'LocalOnly' };

export const MOCK_COMMIT: Commit = {
	id: '1234123',
	parentIds: ['parent-sha'],
	message: 'Initial commit',
	hasConflicts: false,
	state: MOCK_COMMIT_STATE,
	createdAt: 1714000000000,
	author: MOCK_AUTHOR
};

export const MOCK_UPSTREAM_COMMIT: UpstreamCommit = {
	id: 'upstream-sha',
	message: 'Upstream commit',
	createdAt: 1714000000001,
	author: MOCK_AUTHOR
};

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

export const MOCK_STACK_DETAILS: StackDetails = {
	derivedName: 'mock-branch',
	pushStatus: 'completelyUnpushed',
	branchDetails: [MOCK_BRANCH_DETAILS],
	isConflicted: false
};
