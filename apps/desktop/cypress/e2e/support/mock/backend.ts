import { getBaseBranchData } from './baseBranch';
import {
	bytesToStr,
	isGetCommitChangesParams,
	isGetDiffParams,
	isGetWorktreeChangesParams,
	isUndoCommitParams,
	MOCK_TREE_CHANGE_A,
	MOCK_UNIFIED_DIFF
} from './changes';
import {
	isCreateCommitParams,
	isStackDetailsParams,
	isUpdateCommitMessageParams,
	MOCK_BRAND_NEW_BRANCH_NAME,
	MOCK_COMMIT,
	MOCK_STACK_A_ID,
	MOCK_STACK_BRAND_NEW,
	MOCK_STACK_BRAND_NEW_ID,
	MOCK_STACK_DETAILS,
	MOCK_STACK_DETAILS_BRAND_NEW,
	MOCK_STACKS
} from './stacks';
import { MOCK_BRANCH_STATUSES_RESPONSE, MOCK_INTEGRATION_OUTCOME } from './upstreamIntegration';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { TreeChange, TreeChanges, WorktreeChanges } from '$lib/hunks/change';
import type { UnifiedDiff } from '$lib/hunks/diff';
import type { Stack, StackDetails } from '$lib/stacks/stack';
import type { BranchStatusesResponse, IntegrationOutcome } from '$lib/upstream/types';
import type { InvokeArgs } from '@tauri-apps/api/core';

export type MockBackendOptions = {
	initalStacks?: Stack[];
};

type StackId = string;
type CommitId = string;

/**
 * *Ooooh look at me, I'm a mock backend!*
 */
export default class MockBackend {
	protected stacks: Stack[];
	protected stackDetails: Map<StackId, StackDetails>;
	protected commitChanges: Map<CommitId, TreeChange[]>;
	protected worktreeChanges: WorktreeChanges;
	stackId: string = MOCK_STACK_A_ID;
	renamedCommitId: string = '424242424242';
	commitOid: string = MOCK_COMMIT.id;
	cannedBranchName = MOCK_BRAND_NEW_BRANCH_NAME;

	constructor(private options: MockBackendOptions = {}) {
		this.stacks = options.initalStacks ?? MOCK_STACKS;
		this.stackDetails = new Map<string, StackDetails>();
		this.commitChanges = new Map<string, TreeChange[]>();
		this.worktreeChanges = { changes: [MOCK_TREE_CHANGE_A], ignoredChanges: [] };

		this.stackDetails.set(MOCK_STACK_A_ID, structuredClone(MOCK_STACK_DETAILS));
		this.stackDetails.set(MOCK_STACK_BRAND_NEW_ID, structuredClone(MOCK_STACK_DETAILS_BRAND_NEW));
		this.commitChanges.set(MOCK_COMMIT.id, []);
		this.commitChanges.set(this.renamedCommitId, []);
	}

	public getStacks(): Stack[] {
		return this.stacks;
	}

	public getCannedBranchName(): string {
		return this.cannedBranchName ?? 'super-cool-branch-name';
	}

	public createBranch(): Stack {
		this.stacks.push(MOCK_STACK_BRAND_NEW);
		return MOCK_STACK_BRAND_NEW;
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
			const newId = this.renamedCommitId;
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

	public getWorktreeChangesTopLevelDirs(): string[] {
		return this.worktreeChanges.changes
			.map((change) => {
				const listed = change.path.split('/');
				if (listed.length < 2) return undefined;
				return listed[0];
			})
			.filter(isDefined)
			.filter((dir, index, self) => self.indexOf(dir) === index);
	}

	public getWorktreeChangesTopLevelFiles(): string[] {
		return this.worktreeChanges.changes
			.map((change) => {
				const listed = change.path.split('/');
				if (listed.length > 1) return undefined;
				return listed[0];
			})
			.filter(isDefined)
			.filter((file, index, self) => self.indexOf(file) === index);
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
		const remainingChanges: TreeChange[] = [];
		const committedChanges: TreeChange[] = [];

		for (const change of this.worktreeChanges.changes) {
			const isCommitted = worktreeChanges.some((c) => bytesToStr(c.pathBytes) === change.path);
			if (isCommitted) {
				committedChanges.push(change);
			} else {
				remainingChanges.push(change);
			}
		}

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
		this.commitChanges.set(newCommitId, committedChanges);

		const pathsToRejectedChanges: string[] = [];

		return { newCommit: newCommitId, pathsToRejectedChanges };
	}

	public getDiff(args: InvokeArgs | undefined): UnifiedDiff {
		if (!args || !isGetDiffParams(args)) {
			throw new Error('Invalid arguments for getDiff');
		}

		return MOCK_UNIFIED_DIFF;
	}

	public getCommitChanges(args: InvokeArgs | undefined): TreeChanges {
		if (!args || !isGetCommitChangesParams(args)) {
			throw new Error('Invalid arguments for getCommitChanges');
		}

		const { commitId } = args;
		const changes = this.commitChanges.get(commitId);

		if (!changes) {
			throw new Error(`No changes found for commit with ID ${commitId}`);
		}

		return {
			changes,
			stats: {
				linesAdded: 0,
				linesRemoved: 0,
				filesChanged: changes.length
			}
		};
	}

	public undoCommit(args: InvokeArgs | undefined) {
		if (!args || !isUndoCommitParams(args)) {
			throw new Error('Invalid arguments for getCommitChanges');
		}

		const { stackId, commitOid } = args;

		const stackDetails = this.stackDetails.get(stackId);
		if (!stackDetails) {
			throw new Error(`Stack with ID ${stackId} not found`);
		}

		const editableDetails = structuredClone(stackDetails);

		for (const branch of editableDetails.branchDetails) {
			const commitToUndo = branch.commits.find((commit) => commit.id === commitOid);
			if (!commitToUndo) continue;

			branch.commits = branch.commits.filter((commit) => commit.id !== commitOid);
			this.stackDetails.set(stackId, editableDetails);
			// TODO: update the worktree changes
			return;
		}

		throw new Error(`Commit with ID ${commitOid} not found`);
	}

	public getBaseBranchData(): unknown {
		return getBaseBranchData();
	}

	public getUpstreamIntegrationStatuses(): BranchStatusesResponse {
		return MOCK_BRANCH_STATUSES_RESPONSE;
	}

	public integrateUpstream(_args: InvokeArgs | undefined): IntegrationOutcome {
		return MOCK_INTEGRATION_OUTCOME;
	}
}
