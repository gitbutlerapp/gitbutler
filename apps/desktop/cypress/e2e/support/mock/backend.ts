import { bytesToStr, isGetWorktreeChangesParams, MOCK_TREE_CHANGE_A } from './changes';
import {
	isCreateCommitParams,
	isStackDetailsParams,
	isUpdateCommitMessageParams,
	MOCK_COMMIT,
	MOCK_STACK_A_ID,
	MOCK_STACK_DETAILS
} from './stacks';
import type { WorktreeChanges } from '$lib/hunks/change';
import type { StackDetails } from '$lib/stacks/stack';
import type { InvokeArgs } from '@tauri-apps/api/core';

/**
 * *Ooooh look at me, I'm a mock backend!*
 */
export default class MockBackend {
	private stackDetails: Map<string, StackDetails>;
	private worktreeChanges: WorktreeChanges;
	stackId: string = MOCK_STACK_A_ID;
	commitOid: string = MOCK_COMMIT.id;

	constructor() {
		this.stackDetails = new Map<string, StackDetails>();
		this.worktreeChanges = { changes: [MOCK_TREE_CHANGE_A], ignoredChanges: [] };

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

	public getWorktreeChanges(args: InvokeArgs | undefined): WorktreeChanges {
		if (!args || !isGetWorktreeChangesParams(args)) {
			throw new Error('Invalid arguments for getWorktreeChanges');
		}

		return this.worktreeChanges;
	}

	public getWorktreeChangesFileNames(): string[] {
		return this.worktreeChanges.changes
			.map((change) => change.path)
			.map((path) => path.split('/').pop()!);
	}

	public createCommit(args: InvokeArgs | undefined): {
		newCommit: string;
		pathsToRejectedChanges: string[];
	} {
		if (!args || !isCreateCommitParams(args)) {
			throw new Error('Invalid arguments for createCommit' + JSON.stringify(args));
		}

		const { stackId, stackBranchName, message, worktreeChanges } = args;

		const stackDetails = this.stackDetails.get(stackId);
		if (!stackDetails) {
			throw new Error(`Stack with ID ${stackId} not found`);
		}

		const editableDetails = structuredClone(stackDetails);

		// Assume only full file changes are passed.
		const remainingChanges = this.worktreeChanges.changes.filter((change) => {
			return !worktreeChanges.some((c) => bytesToStr(c.pathBytes) === change.path);
		});

		this.worktreeChanges = {
			...this.worktreeChanges,
			changes: remainingChanges
		};

		const branch = editableDetails.branchDetails.find((b) => b.name === stackBranchName);

		if (!branch) {
			throw new Error(`Branch with name ${stackBranchName} not found`);
		}

		const topCommit = branch.commits[branch.commits.length - 1];
		const parentIds = topCommit ? [topCommit.id] : [];

		const newCommitId = 'new-commit-id';

		branch.commits = [
			{
				...MOCK_COMMIT,
				message,
				parentIds,
				createdAt: Date.now(),
				id: newCommitId
			},
			...branch.commits
		];

		this.stackDetails.set(stackId, editableDetails);

		const pathsToRejectedChanges: string[] = [];

		return { newCommit: newCommitId, pathsToRejectedChanges };
	}
}
