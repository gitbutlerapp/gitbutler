import { mockReduxFulfilled } from '$lib/testing/mockRedux';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { vi } from 'vitest';
import type { Author, Commit, UpstreamCommit } from '$lib/branches/v3';
import type { BranchDetails } from '$lib/stacks/stack';

const MOCK_AUTHOR_A: Author = {
	name: 'Author A',
	email: 'some-email@greatemail.com',
	gravatarUrl: 'https://example.com/avatar-a.png'
};

const MOCK_COMMIT_A: Commit = {
	id: 'commit-a-id',
	parentIds: ['parent-commit-id'],
	message: 'Initial commit message',
	hasConflicts: false,
	state: { type: 'LocalOnly' },
	createdAt: BigInt(1672531200000), // Example timestamp
	author: MOCK_AUTHOR_A,
	gerritReviewUrl: null
};

const MOCK_UPSTREAM_COMMIT_A: UpstreamCommit = {
	id: 'upstream-commit-a-id',
	message: 'Upstream commit message',
	createdAt: BigInt(1672531200000), // Example timestamp
	author: MOCK_AUTHOR_A
};

const BRANCH_DETAILS_A: BranchDetails = {
	name: 'branch-a',
	pushStatus: 'nothingToPush',
	lastUpdatedAt: BigInt(1672531200000), // Example timestamp
	authors: [MOCK_AUTHOR_A],
	isConflicted: false,
	commits: [MOCK_COMMIT_A],
	upstreamCommits: [MOCK_UPSTREAM_COMMIT_A],
	remoteTrackingBranch: null,
	description: null,
	prNumber: null,
	reviewId: null,
	tip: 'tip-commit-a',
	baseCommit: 'base-commit-a',
	isRemoteHead: false,
	linkedWorktreeId: null
};

export function getStackServiceMock() {
	const StackServiceMock = vi.fn();

	StackServiceMock.prototype.stacks = vi.fn();
	StackServiceMock.prototype.stackAt = vi.fn();
	StackServiceMock.prototype.stackById = vi.fn();
	StackServiceMock.prototype.defaultBranch = vi.fn();
	StackServiceMock.prototype.stackInfo = vi.fn();
	StackServiceMock.prototype.branchDetails = vi.fn(() => {
		return reactive(() => mockReduxFulfilled(BRANCH_DETAILS_A));
	});
	StackServiceMock.prototype.branches = vi.fn(() => {
		return reactive(() => mockReduxFulfilled([BRANCH_DETAILS_A]));
	});
	StackServiceMock.prototype.branchAt = vi.fn();
	StackServiceMock.prototype.branchParentByName = vi.fn();
	StackServiceMock.prototype.branchChildByName = vi.fn();
	StackServiceMock.prototype.branchByName = vi.fn((_projectId, _stackId, branchName) => {
		return reactive(() => mockReduxFulfilled({ ...BRANCH_DETAILS_A, name: branchName }));
	});
	StackServiceMock.prototype.commits = vi.fn(() => {
		return reactive(() => mockReduxFulfilled([MOCK_COMMIT_A]));
	});
	StackServiceMock.prototype.commitAt = vi.fn(() => {
		return reactive(() => mockReduxFulfilled(MOCK_COMMIT_A));
	});
	StackServiceMock.prototype.commitById = vi.fn();
	StackServiceMock.prototype.upstreamCommits = vi.fn(() => {
		return reactive(() => mockReduxFulfilled([MOCK_UPSTREAM_COMMIT_A]));
	});
	StackServiceMock.prototype.upstreamCommitAt = vi.fn();
	StackServiceMock.prototype.upstreamCommitById = vi.fn();
	StackServiceMock.prototype.commitChanges = vi.fn();
	StackServiceMock.prototype.commitChange = vi.fn();
	StackServiceMock.prototype.branchChanges = vi.fn();
	StackServiceMock.prototype.branchChange = vi.fn();

	StackServiceMock.prototype.newStack = vi.fn();
	StackServiceMock.prototype.newStackMutation = vi.fn();
	StackServiceMock.prototype.createStack = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.updateStack = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.updateStackOrder = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.pushStack = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.createCommit = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.createCommitLegacy = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.updateCommitMessage = [
		vi.fn(),
		reactive(() => mockReduxFulfilled({}))
	];
	StackServiceMock.prototype.newBranch = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.uncommit = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.insertBlankCommit = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.unapply = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.publishBranch = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.amendCommit = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.discardChanges = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.updateBranchPrNumber = [
		vi.fn(),
		reactive(() => mockReduxFulfilled({}))
	];
	StackServiceMock.prototype.updateBranchName = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.removeBranch = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.updateBranchDescription = [
		vi.fn(),
		reactive(() => mockReduxFulfilled({}))
	];
	StackServiceMock.prototype.reorderStack = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.reorderStackMutation = vi.fn();
	StackServiceMock.prototype.moveCommit = [vi.fn(), reactive(() => mockReduxFulfilled({}))];
	StackServiceMock.prototype.moveCommitMutation = vi.fn();
	StackServiceMock.prototype.integrateUpstreamCommits = vi.fn();
	StackServiceMock.prototype.legacyUnapplyLines = vi.fn();
	StackServiceMock.prototype.legacyUnapplyHunk = vi.fn();
	StackServiceMock.prototype.legacyUnapplyFiles = vi.fn();
	StackServiceMock.prototype.legacyUpdateBranchOwnership = vi.fn();
	StackServiceMock.prototype.legacyUpdateBranchOwnershipMutation = vi.fn();
	StackServiceMock.prototype.createVirtualBranchFromBranch = vi.fn();
	StackServiceMock.prototype.deleteLocalBranch = vi.fn();
	StackServiceMock.prototype.markResolved = vi.fn();
	StackServiceMock.prototype.squashCommits = vi.fn();
	StackServiceMock.prototype.squashCommitsMutation = vi.fn();
	StackServiceMock.prototype.amendCommitMutation = vi.fn();
	StackServiceMock.prototype.moveCommitFileMutation = vi.fn();

	return StackServiceMock;
}
