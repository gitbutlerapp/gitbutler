import { getBaseBranchData, type BaseBranchData } from './baseBranch';
import { isGetBranchDetailsParams, MOCK_BRANCH_LISTINGS } from './branches';
import {
	bytesToStr,
	isGetBranchChangesParams,
	isGetCommitChangesParams,
	isGetDiffParams,
	isGetWorktreeChangesParams,
	isUndoCommitParams,
	MOCK_TREE_CHANGE_A,
	MOCK_UNIFIED_DIFF
} from './changes';
import { PROJECT_ID } from './projects';
import { isAddRemoteParams } from './remote';
import { getMockTemplateContent, isGetReviewTemplateParams } from './review';
import {
	createMockBranchDetails,
	createMockStack,
	createMockStackDetails,
	isCreateBranchParams,
	isCreateCommitParams,
	isCreateStackParams,
	isCreateVirtualBranchFromBranchParams,
	isDeleteLocalBranchParams,
	isGetTargetCommitsParams,
	isIntegrateUpstreamCommitsParams,
	isPushStackParams,
	isRemoveBranchParams,
	isStackDetailsParams,
	isUpdateBranchNameParams,
	isUpdateBranchPRNumberParams,
	isUpdateCommitMessageParams,
	MOCK_BRAND_NEW_BRANCH_NAME,
	MOCK_COMMIT,
	MOCK_STACK_A_ID,
	MOCK_STACK_BRAND_NEW_ID,
	MOCK_STACK_DETAILS,
	MOCK_STACK_DETAILS_BRAND_NEW,
	MOCK_STACKS
} from './stacks';
import { MOCK_BRANCH_STATUSES_RESPONSE, MOCK_INTEGRATION_OUTCOME } from './upstreamIntegration';
import type { BranchListing } from '$lib/branches/branchListing';
import type { Commit } from '$lib/branches/v3';
import type { HookStatus } from '$lib/hooks/hooksService';
import type { TreeChange, TreeChanges, WorktreeChanges } from '$lib/hunks/change';
import type { UnifiedDiff } from '$lib/hunks/diff';
import type { HunkAssignment } from '$lib/hunks/hunk';
import type { GitRemote } from '$lib/remotes/remotesService';
import type { BranchDetails, Stack, StackDetails } from '$lib/stacks/stack';
import type { BranchPushResult } from '$lib/stacks/stackService.svelte';
import type { BranchStatusesResponse, IntegrationOutcome } from '$lib/upstream/types';
import type { InvokeArgs } from '@tauri-apps/api/core';

export type MockBackendOptions = {
	initalStacks?: Stack[];
};

export type StackId = string;
export type CommitId = string;
export type BranchName = string;
export type BranchChanges = Map<BranchName, TreeChange[]>;

/**
 * *Ooooh look at me, I'm a mock backend!*
 */
export default class MockBackend {
	protected stacks: Stack[];
	protected stackDetails: Map<StackId, StackDetails>;
	protected commitChanges: Map<CommitId, TreeChange[]>;
	protected branchChanges: Map<StackId, BranchChanges>;
	protected worktreeChanges: WorktreeChanges;
	protected unifiedDiffs: Map<string, UnifiedDiff>;
	protected branchListings: BranchListing[];
	protected baseBranchCommits: Commit[];

	stackId: string = MOCK_STACK_A_ID;
	renamedCommitId: string = '424242424242';
	commitId: string = MOCK_COMMIT.id;
	cannedBranchName = MOCK_BRAND_NEW_BRANCH_NAME;
	branchListing: BranchListing;

	constructor(private options: MockBackendOptions = {}) {
		this.stacks = options.initalStacks ?? MOCK_STACKS;
		this.stackDetails = new Map<string, StackDetails>();
		this.commitChanges = new Map<string, TreeChange[]>();
		this.branchChanges = new Map<string, BranchChanges>();
		this.worktreeChanges = {
			changes: [MOCK_TREE_CHANGE_A],
			ignoredChanges: [],
			assignments: [],
			assignmentsError: null,
			dependencies: {
				diffs: [],
				errors: []
			},
			dependenciesError: null
		};
		this.unifiedDiffs = new Map<string, UnifiedDiff>();

		this.branchListings = MOCK_BRANCH_LISTINGS;
		this.branchListing = MOCK_BRANCH_LISTINGS[0]!;
		this.baseBranchCommits = [];

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

	public createBranch(args: InvokeArgs | undefined): Stack {
		if (!args || !isCreateStackParams(args)) {
			throw new Error('Invalid arguments for createBranch');
		}
		const { branch } = args;
		const { name } = branch;
		if (!name) throw new Error('Branch name is required');

		const stack = createMockStack({
			heads: [
				{
					name,
					tip: 'werwer'
				}
			],
			id: name
		});
		this.stacks.push(stack);
		const stackDetails = createMockStackDetails({
			branchDetails: [createMockBranchDetails({ name, commits: [] })],
			derivedName: name,
			pushStatus: 'completelyUnpushed'
		});
		this.stackDetails.set(name, stackDetails);
		return stack;
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
		const { stackId, commitId, message } = args;

		const stackDetails = this.stackDetails.get(stackId);
		if (!stackDetails) {
			throw new Error(`Stack with ID ${stackId} not found`);
		}

		const editableDetails = structuredClone(stackDetails);

		for (const branch of editableDetails.branchDetails) {
			const commitIndex = branch.commits.findIndex((commit) => commit.id === commitId);
			if (commitIndex === -1) continue;
			const commit = branch.commits[commitIndex]!;
			const newId = this.renamedCommitId + message;
			branch.commits[commitIndex] = {
				...commit,
				message,
				id: newId
			};
			branch.pushStatus = 'unpushedCommitsRequiringForce';
			editableDetails.pushStatus = 'unpushedCommitsRequiringForce';
			this.stackDetails.set(stackId, editableDetails);
			this.commitChanges.set(newId, []);
			return newId;
		}

		throw new Error(`Commit with ID ${commitId} not found`);
	}

	public getWorktreeChanges(args: InvokeArgs | undefined): WorktreeChanges {
		if (!args || !isGetWorktreeChangesParams(args)) {
			throw new Error('Invalid arguments for getWorktreeChanges');
		}

		return { ...this.worktreeChanges, assignments: this.getHunkAssignments(args) };
	}

	public getHunkAssignments(args: InvokeArgs | undefined): HunkAssignment[] {
		if (!args || !isGetWorktreeChangesParams(args)) {
			throw new Error('Invalid arguments for getHunkAssignments');
		}

		const out = [];

		for (const change of this.worktreeChanges.changes) {
			if (change.status.type === 'Addition' || change.status.type === 'Deletion') {
				out.push({
					id: 'asdf',
					hunkHeader: null,
					path: change.path,
					pathBytes: change.pathBytes,
					stackId: null,
					lineNumsAdded: [],
					lineNumsRemoved: []
				});
			} else if (change.status.type === 'Rename' || change.status.type === 'Modification') {
				const diff = this.getDiff({ projectId: args.projectId, change });
				if (diff) {
					if (diff.type === 'Binary' || diff.type === 'TooLarge') {
						out.push({
							id: 'asdf',
							hunkHeader: null,
							path: change.path,
							pathBytes: change.pathBytes,
							stackId: null,
							lineNumsAdded: [],
							lineNumsRemoved: []
						});
					} else {
						for (const hunk of diff.subject.hunks) {
							out.push({
								id: 'asdf',
								hunkHeader: hunk,
								path: change.path,
								pathBytes: change.pathBytes,
								stackId: null,
								lineNumsAdded: [],
								lineNumsRemoved: []
							});
						}
					}
				} else {
					out.push({
						id: 'asdf',
						hunkHeader: null,
						path: change.path,
						pathBytes: change.pathBytes,
						stackId: null,
						lineNumsAdded: [],
						lineNumsRemoved: []
					});
				}
			}
		}

		return out;
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
			.filter((dir): dir is string => !!dir)
			.filter((dir, index, self) => self.indexOf(dir) === index);
	}

	public getWorktreeChangesTopLevelFiles(): string[] {
		return this.worktreeChanges.changes
			.map((change) => {
				const listed = change.path.split('/');
				if (listed.length > 1) return undefined;
				return listed[0];
			})
			.filter((dir): dir is string => !!dir)
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

		const newCommitId = 'new-commit-id' + message;

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

		const diff = this.unifiedDiffs.get(args.change.path);

		return diff ?? MOCK_UNIFIED_DIFF;
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

		const { stackId, commitId } = args;

		const stackDetails = this.stackDetails.get(stackId);
		if (!stackDetails) {
			throw new Error(`Stack with ID ${stackId} not found`);
		}

		const editableDetails = structuredClone(stackDetails);

		for (const branch of editableDetails.branchDetails) {
			const commitToUndo = branch.commits.find((commit) => commit.id === commitId);
			if (!commitToUndo) continue;

			branch.commits = branch.commits.filter((commit) => commit.id !== commitId);
			this.stackDetails.set(stackId, editableDetails);
			// TODO: update the worktree changes
			return;
		}

		throw new Error(`Commit with ID ${commitId} not found`);
	}

	public getBaseBranchData(_: InvokeArgs | undefined): BaseBranchData | null {
		return getBaseBranchData();
	}

	public getBaseBranchName(): string | undefined {
		const baseBranch = this.getBaseBranchData(undefined);
		return baseBranch?.branchName;
	}

	public getBaseBranchCommits(args: InvokeArgs | undefined): Commit[] {
		if (!args || !isGetTargetCommitsParams(args)) {
			throw new Error('Invalid arguments for getBaseBranchCommits');
		}

		const { lastCommitId, pageSize } = args;
		const baseBranchCommits = this.baseBranchCommits;

		const lastCommitIndex = baseBranchCommits.findIndex((commit) => commit.id === lastCommitId);
		const startIndex = lastCommitIndex === -1 ? 0 : lastCommitIndex + 1;
		const endIndex = Math.min(startIndex + pageSize, baseBranchCommits.length);

		return baseBranchCommits.slice(startIndex, endIndex);
	}

	public getUpstreamIntegrationStatuses(): BranchStatusesResponse {
		return MOCK_BRANCH_STATUSES_RESPONSE;
	}

	public integrateUpstream(_args: InvokeArgs | undefined): IntegrationOutcome {
		return MOCK_INTEGRATION_OUTCOME;
	}

	public getBranchChanges(args: InvokeArgs | undefined): TreeChanges {
		if (!args || !isGetBranchChangesParams(args)) {
			throw new Error('Invalid arguments for getBranchChanges');
		}

		const { stackId, branchName } = args;

		if (!stackId) {
			return {
				changes: [],
				stats: {
					linesAdded: 0,
					linesRemoved: 0,
					filesChanged: 0
				}
			};
		}

		const stackBranchChanges = this.branchChanges.get(stackId);
		if (!stackBranchChanges) {
			throw new Error(`No changes found for stack with ID ${stackId}`);
		}

		const branchChanges = stackBranchChanges.get(branchName);
		if (!branchChanges) {
			throw new Error(`No changes found for branch with name ${branchName}`);
		}

		return {
			changes: branchChanges,
			stats: {
				linesAdded: 0,
				linesRemoved: 0,
				filesChanged: branchChanges.length
			}
		};
	}

	public getBranchChangesFileNames(
		stackId: string,
		branchName: string,
		projectId: string = PROJECT_ID
	): string[] {
		const changes = this.getBranchChanges({ projectId, stackId, branchName });
		return changes.changes.map((change) => change.path).map((path) => path.split('/').pop()!);
	}

	public listBranches(args: InvokeArgs | undefined): BranchListing[] {
		if (!args) {
			throw new Error('Invalid arguments for listBranches');
		}
		return this.branchListings;
	}

	public getBranchDetails(args: InvokeArgs | undefined): BranchDetails {
		if (!args || !isGetBranchDetailsParams(args)) {
			throw new Error('Invalid arguments for getBranchDetails');
		}

		const { branchName } = args;

		const baseBranch = this.getBaseBranchData(undefined);
		if (!baseBranch) {
			throw new Error('Base branch data not found');
		}
		const baseBranchName = baseBranch.branchName.replace(`${baseBranch.remoteName}/`, '');
		if (baseBranchName === branchName) {
			return createMockBranchDetails({
				...baseBranch,
				name: baseBranch.branchName
			});
		}

		const maybeBranchListing = this.branchListings.find((b) => b.name === branchName);

		if (maybeBranchListing) {
			return createMockBranchDetails({
				...maybeBranchListing
			});
		}

		for (const stack of this.stackDetails.values()) {
			const branch = stack.branchDetails.find((b) => b.name === branchName);
			if (branch) return branch;
		}

		throw new Error(`Branch with name ${branchName} not found`);
	}

	public renameBranch(args: InvokeArgs | undefined): void {
		if (!args || !isUpdateBranchNameParams(args)) {
			throw new Error('Invalid arguments for renameBranch');
		}

		const { stackId, branchName, newName } = args;
		const stackDetails = this.stackDetails.get(stackId);
		if (!stackDetails) {
			throw new Error(`Stack with ID ${stackId} not found`);
		}

		const editableDetails = structuredClone(stackDetails);

		const branchIndex = editableDetails.branchDetails.findIndex((b) => b.name === branchName);
		if (branchIndex === -1) {
			throw new Error(`Branch with name ${branchName} not found`);
		}

		const branch = editableDetails.branchDetails[branchIndex]!;
		editableDetails.branchDetails[branchIndex] = {
			...branch,
			name: newName
		};

		this.stackDetails.set(stackId, editableDetails);
		this.branchListings = this.branchListings.map((branch) => {
			if (branch.name === branchName) {
				return {
					...branch,
					name: newName
				};
			}
			return branch;
		});
		this.branchListing = this.branchListings.find((branch) => branch.name === newName)!;
	}

	public removeBranch(args: InvokeArgs | undefined): void {
		if (!args || !isRemoveBranchParams(args)) {
			throw new Error('Invalid arguments for removeBranch');
		}

		const { stackId, branchName } = args;
		const stackDetails = this.stackDetails.get(stackId);
		if (!stackDetails) {
			throw new Error(`Stack with ID ${stackId} not found`);
		}
		const editableDetails = structuredClone(stackDetails);
		const branchIndex = editableDetails.branchDetails.findIndex((b) => b.name === branchName);
		if (branchIndex === -1) {
			throw new Error(`Branch with name ${branchName} not found`);
		}

		editableDetails.branchDetails.splice(branchIndex, 1);
		this.stackDetails.set(stackId, editableDetails);
		this.branchListings = this.branchListings.filter((branch) => branch.name !== branchName);
		this.branchListing = this.branchListings.find((branch) => branch.name === branchName)!;
	}

	public addBranch(args: InvokeArgs | undefined): void {
		if (!args || !isCreateBranchParams(args)) {
			throw new Error('Invalid arguments for addBranch');
		}

		const { stackId, request } = args;

		const stackDetails = this.stackDetails.get(stackId);
		if (!stackDetails) {
			throw new Error(`Stack with ID ${stackId} not found`);
		}

		const editableDetails = structuredClone(stackDetails);

		editableDetails.branchDetails.splice(0, 0, createMockBranchDetails({ name: request.name }));

		this.stackDetails.set(stackId, editableDetails);
	}

	public createVirtualBranchFromBranch(args: InvokeArgs | undefined) {
		if (!args || !isCreateVirtualBranchFromBranchParams(args)) {
			throw new Error('Invalid arguments for createVirtualBranchFromBranch');
		}

		// Do nothing for now
	}

	public deleteLocalBranch(args: InvokeArgs | undefined) {
		if (!args || !isDeleteLocalBranchParams(args)) {
			throw new Error('Invalid arguments for deleteLocalBranch');
		}

		// Do nothing for now
	}

	public integrateUpstreamCommits(args: InvokeArgs | undefined) {
		if (!args || !isIntegrateUpstreamCommitsParams(args)) {
			throw new Error('Invalid arguments for integrateUpstreamCommits');
		}

		// Do nothing for now
	}

	public getAvailableReviewTemplates(): string[] {
		return [];
	}

	public pushStack(args: InvokeArgs | undefined): BranchPushResult {
		if (!args || !isPushStackParams(args)) {
			throw new Error('Invalid arguments for pushStack');
		}

		// Make the branches local and remote
		const stackDetails = this.stackDetails.get(args.stackId);
		if (!stackDetails) {
			throw new Error(`Stack with ID ${args.stackId} not found`);
		}

		const editableDetails = structuredClone(stackDetails);

		for (const branch of editableDetails.branchDetails) {
			branch.commits = branch.commits.map((commit) => ({
				...commit,
				state: {
					type: 'LocalAndRemote',
					subject: commit.id
				}
			}));
			branch.pushStatus = 'nothingToPush';
		}

		editableDetails.pushStatus = 'nothingToPush';

		this.stackDetails.set(args.stackId, editableDetails);

		return {
			refname: `refs/remotes/origin/${args.branch}`,
			remote: 'origin'
		};
	}

	public listRemotes(args: InvokeArgs | undefined): GitRemote[] {
		if (!args) {
			throw new Error('Invalid arguments for listRemotes');
		}

		return [
			{
				name: 'origin',
				url: ''
			}
		];
	}

	public updateBranchPrNumber(args: InvokeArgs | undefined): void {
		if (!args || !isUpdateBranchPRNumberParams(args)) {
			throw new Error('Invalid arguments for updateBranchPrNumber');
		}

		const { stackId, branchName, prNumber } = args;

		const stackDetails = this.stackDetails.get(stackId);
		if (!stackDetails) {
			throw new Error(`Stack with ID ${stackId} not found`);
		}

		const editableDetails = structuredClone(stackDetails);
		const branchIndex = editableDetails.branchDetails.findIndex((b) => b.name === branchName);
		if (branchIndex === -1) {
			throw new Error(`Branch with name ${branchName} not found`);
		}

		const branch = editableDetails.branchDetails[branchIndex]!;
		editableDetails.branchDetails[branchIndex] = {
			...branch,
			prNumber: prNumber
		};

		this.stackDetails.set(stackId, editableDetails);
	}

	public addRemote(args: InvokeArgs | undefined): string {
		if (!args || !isAddRemoteParams(args)) {
			throw new Error('Invalid arguments for addRemote');
		}

		const { name } = args;

		return `refs/remotes/${name}`;
	}

	public getTemplateContent(args: InvokeArgs | undefined): string {
		if (!args || !isGetReviewTemplateParams(args)) {
			throw new Error('Invalid arguments for getTemplateContent');
		}

		return getMockTemplateContent();
	}

	public async precommitHookDiffspecs(waitTime: number): Promise<HookStatus> {
		return await new Promise((resolve) => {
			setTimeout(() => {
				resolve({ status: 'success' });
			}, waitTime);
		});
	}

	public async postcommitHook(waitTime: number): Promise<HookStatus> {
		return await new Promise((resolve) => {
			setTimeout(() => {
				resolve({ status: 'success' });
			}, waitTime);
		});
	}
}
