import type { Author, Commit, CommitState, UpstreamCommit } from '$lib/branches/v3';
import type { BranchDetails, Stack, StackDetails } from '$lib/stacks/stack';
import type { InvokeArgs } from '@tauri-apps/api/core';

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

/**
 * *Ohhh, look at me, I'm a stack service!*
 */
export class MockStackService {
	private stackDetails: Map<string, StackDetails>;
	stackId: string = MOCK_STACK_A_ID;
	commitOid: string = MOCK_COMMIT.id;

	constructor() {
		this.stackDetails = new Map<string, StackDetails>();

		this.stackDetails.set(MOCK_STACK_A_ID, structuredClone(MOCK_STACK_DETAILS));
	}

	public getStackDetails(args: InvokeArgs | undefined): StackDetails {
		if (!args || !isStackDetailsParams(args)) {
			throw new Error('Invalid arguments for getStackDetails');
		}
		const { stackId } = args;
		const stackDetails = this.stackDetails.get(stackId);
		if (!stackDetails) {
			throw new Error(`Stack with ID ${stackId} not found`);
		}
		return stackDetails;
	}

	public updateCommitMessage(args: InvokeArgs | undefined): string {
		if (!args || !isUpdateCommitMessageParams(args)) {
			throw new Error('Invalid arguments for renameCommit');
		}
		const { stackId, commitOid, message } = args;

		const stackDetails = this.stackDetails.get(stackId);
		if (!stackDetails) {
			throw new Error(`Stack with ID ${stackId} not found`);
		}

		const editableDetails = structuredClone(stackDetails);

		for (const branch of editableDetails.branchDetails) {
			const commitIndex = branch.commits.findIndex((commit) => commit.id === commitOid);
			if (commitIndex === -1) continue;
			const commit = branch.commits[commitIndex]!;
			const newId = '424242424242';
			branch.commits[commitIndex] = {
				...commit,
				message,
				id: newId
			};
			this.stackDetails.set(stackId, editableDetails);
			return newId;
		}

		throw new Error(`Commit with ID ${commitOid} not found`);
	}
}
