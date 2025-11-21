import MockBackend from '../mock/backend';
import { getBaseBranchBehindData } from '../mock/baseBranch';
import {
	createMockCommit,
	MOCK_BRANCH_DETAILS,
	MOCK_COMMIT_STATE_INTEGRATED
} from '../mock/stacks';
import type { BranchStatusesResponse } from '$lib/upstream/types';
import type { Workspace, WorkspaceLegacy } from '@gitbutler/core/api';

const MOCK_STACK_A_ID = 'stack-a-id';
const MOCK_STACK_B_ID = 'stack-b-id';
const MOCK_STACK_C_ID = 'stack-c-id';

const MOCK_STACK_A: WorkspaceLegacy.StackEntry = {
	order: 0,
	id: MOCK_STACK_A_ID,
	heads: [{ name: MOCK_STACK_A_ID, tip: '1234123', isCheckedOut: true }],
	tip: '1234123',
	isCheckedOut: true
};

const MOCK_STACK_B: WorkspaceLegacy.StackEntry = {
	order: 1,
	id: MOCK_STACK_B_ID,
	heads: [
		{ name: MOCK_STACK_B_ID, tip: '1234123', isCheckedOut: true },
		{ name: 'branch-d', tip: '456456456', isCheckedOut: true }
	],
	tip: '1234123',
	isCheckedOut: true
};

const MOCK_STACK_C: WorkspaceLegacy.StackEntry = {
	order: 2,
	id: MOCK_STACK_C_ID,
	heads: [
		{ name: MOCK_STACK_C_ID, tip: '1234123', isCheckedOut: true },
		{ name: 'branch-e', tip: '456456456', isCheckedOut: true }
	],
	tip: '1234123',
	isCheckedOut: true
};

const MOCK_BRANCH_DETAILS_A: Workspace.BranchDetails = {
	...MOCK_BRANCH_DETAILS,
	name: MOCK_STACK_A_ID,
	tip: '1234123',
	remoteTrackingBranch: `origin/${MOCK_STACK_A_ID}`,
	pushStatus: 'unpushedCommits',
	commits: [
		createMockCommit({ id: '1234123', message: 'Commit 1' }),
		createMockCommit({
			id: '456456456',
			message: 'Commit 2',
			state: { type: 'LocalAndRemote', subject: '77888777888' }
		}),
		createMockCommit({
			id: '789789789',
			message: 'Commit 3',
			state: { type: 'LocalAndRemote', subject: '789789789' }
		})
	]
};

const MOCK_STACK_DETAILS_A: Workspace.StackDetails = {
	derivedName: MOCK_STACK_A_ID,
	pushStatus: 'unpushedCommits',
	branchDetails: [MOCK_BRANCH_DETAILS_A],
	isConflicted: false
};

const MOCK_BRANCH_DETAILS_B: Workspace.BranchDetails = {
	...MOCK_BRANCH_DETAILS,
	name: MOCK_STACK_B_ID,
	tip: '1234123',
	remoteTrackingBranch: `origin/${MOCK_STACK_B_ID}`,
	pushStatus: 'integrated',
	commits: [
		createMockCommit({
			id: '1234123',
			message: 'Commit 1 (B)',
			state: MOCK_COMMIT_STATE_INTEGRATED
		}),
		createMockCommit({
			id: '456456456',
			message: 'Commit 2 (B)',
			state: MOCK_COMMIT_STATE_INTEGRATED
		})
	]
};

const MOCK_BRANCH_DETAILS_B_D: Workspace.BranchDetails = {
	...MOCK_BRANCH_DETAILS,
	name: 'branch-d',
	tip: '456456456',
	remoteTrackingBranch: 'origin/branch-d',
	pushStatus: 'integrated',
	commits: [
		createMockCommit({
			id: '456456456',
			message: 'Commit 1 (branch-d)',
			state: MOCK_COMMIT_STATE_INTEGRATED
		})
	]
};

const MOCK_STACK_DETAILS_B: Workspace.StackDetails = {
	derivedName: MOCK_STACK_B_ID,
	pushStatus: 'integrated',
	branchDetails: [MOCK_BRANCH_DETAILS_B, MOCK_BRANCH_DETAILS_B_D],
	isConflicted: false
};

const MOCK_BRANCH_DETAILS_C: Workspace.BranchDetails = {
	...MOCK_BRANCH_DETAILS,
	name: MOCK_STACK_C_ID,
	tip: '1234123',
	remoteTrackingBranch: `origin/${MOCK_STACK_C_ID}`,
	pushStatus: 'integrated',
	commits: [
		createMockCommit({
			id: '1234123',
			message: 'Commit 1 (C)',
			state: MOCK_COMMIT_STATE_INTEGRATED
		}),
		createMockCommit({
			id: '456456456',
			message: 'Commit 2 (C)',
			state: MOCK_COMMIT_STATE_INTEGRATED
		})
	]
};

const MOCK_STACK_DETAILS_C: Workspace.StackDetails = {
	derivedName: MOCK_STACK_C_ID,
	pushStatus: 'unpushedCommits',
	branchDetails: [MOCK_BRANCH_DETAILS_C],
	isConflicted: false
};

const MOCK_BRANCH_STATUSES_RESPONSE: BranchStatusesResponse = {
	type: 'updatesRequired',
	subject: {
		worktreeConflicts: [],
		statuses: [
			[
				MOCK_STACK_A_ID,
				{
					treeStatus: { type: 'saflyUpdatable' },
					branchStatuses: [{ name: MOCK_STACK_A_ID, status: { type: 'saflyUpdatable' } }]
				}
			],
			[
				MOCK_STACK_B_ID,
				{
					treeStatus: { type: 'empty' },
					branchStatuses: [
						{ name: 'branch-d', status: { type: 'integrated' } },
						{ name: MOCK_STACK_B_ID, status: { type: 'integrated' } }
					]
				}
			],
			[
				MOCK_STACK_C_ID,
				{
					treeStatus: { type: 'empty' },
					branchStatuses: [{ name: MOCK_STACK_C_ID, status: { type: 'integrated' } }]
				}
			]
		]
	}
};

/**
 * This is a mock backend for the following scenario:
 *
 * There are multiple stacks in the workspace.
 * Some of the stacks are fully integrated, while others are partially integrated.
 *
 * The base branch is one commit behind.
 */
export default class PartiallyIntegratedBranches extends MockBackend {
	constructor() {
		super();
		this.stacks = [MOCK_STACK_A, MOCK_STACK_B, MOCK_STACK_C];
		this.stackId = MOCK_STACK_A_ID;
		this.stackDetails = new Map<string, Workspace.StackDetails>();
		this.stackDetails.set(MOCK_STACK_A_ID, MOCK_STACK_DETAILS_A);
		this.stackDetails.set(MOCK_STACK_B_ID, MOCK_STACK_DETAILS_B);
		this.stackDetails.set(MOCK_STACK_C_ID, MOCK_STACK_DETAILS_C);
	}

	public getBaseBranchData() {
		return getBaseBranchBehindData() as any;
	}

	public getUpstreamIntegrationStatuses(): BranchStatusesResponse {
		return MOCK_BRANCH_STATUSES_RESPONSE;
	}
}
