import { ConflictEntries, type ConflictEntriesObj } from '$lib/files/conflicts';
import { sortLikeFileTree } from '$lib/files/filetreeV3';
import { showToast } from '$lib/notifications/toasts';
import { hasBackendExtra } from '$lib/state/backendQuery';
import { ClientState, type BackendApi } from '$lib/state/clientState.svelte';
import { createSelectByIds, createSelectNth } from '$lib/state/customSelectors';
import {
	invalidatesItem,
	invalidatesList,
	providesItem,
	providesList,
	ReduxTag
} from '$lib/state/tags';
import {
	replaceBranchInExclusiveAction,
	replaceBranchInStackSelection,
	updateStaleProjectState,
	type UiState,
	updateStaleStackState
} from '$lib/state/uiState.svelte';
import { getBranchNameFromRef, type BranchRef } from '$lib/utils/branch';
import { InjectionToken } from '@gitbutler/core/context';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import {
	createEntityAdapter,
	type EntityState,
	type ThunkDispatch,
	type UnknownAction
} from '@reduxjs/toolkit';
import type { StackOrder } from '$lib/branches/branch';
import type { Commit, CommitDetails, UpstreamCommit } from '$lib/branches/v3';
import type { MoveCommitIllegalAction } from '$lib/commits/commit';
import type { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
import type { TreeChange, TreeChanges, TreeStats } from '$lib/hunks/change';
import type { DiffSpec } from '$lib/hunks/hunk';
import type {
	BranchDetails,
	Stack,
	StackDetails,
	CreateRefRequest,
	InteractiveIntegrationStep
} from '$lib/stacks/stack';
import type { ReduxError } from '$lib/state/reduxError';

type BranchParams = {
	name?: string;
	ownership?: string;
	order?: number;
	allow_rebasing?: boolean;
	notes?: string;
	selected_for_changes?: boolean;
};

export type CreateCommitRequest = {
	stackId: string;
	message: string;
	/** Undefined means that the backend will infer the parent to be the current head of stackBranchName */
	parentId: string | undefined;
	stackBranchName: string;
	worktreeChanges: DiffSpec[];
};

export type CreateCommitRequestWorktreeChanges = DiffSpec;

type StackAction = 'push';

type StackErrorInfo = {
	title: string;
	codeInfo: Record<string, string>;
	defaultInfo: string;
};

const ERROR_INFO: Record<StackAction, StackErrorInfo> = {
	push: {
		title: 'Git push failed',
		codeInfo: {
			['errors.git.authentication']: 'an authentication failure',
			['errors.git.force_push_protection']: 'force push protection'
		},
		defaultInfo: 'an unforeseen error'
	}
};

function surfaceStackError(action: StackAction, errorCode: string, errorMessage: string): boolean {
	const reason = ERROR_INFO[action].codeInfo[errorCode] ?? ERROR_INFO[action].defaultInfo;
	const title = ERROR_INFO[action].title;
	switch (action) {
		case 'push': {
			if (errorCode === 'errors.git.force_push_protection') {
				return false;
			}

			showToast({
				title,
				message: `
Your branch cannot be pushed due to ${reason}.

Please check our [documentation](https://docs.gitbutler.com/troubleshooting/fetch-push)
on fetching and pushing for ways to resolve the problem.
			`.trim(),
				error: errorMessage,
				style: 'error'
			});

			return true;
		}
	}
}

export type SeriesIntegrationStrategy = 'merge' | 'rebase';

export interface BranchPushResult {
	/**
	 * The list of pushed branches and their corresponding remote refnames.
	 */
	branchToRemote: [string, string][];
	/**
	 * The name of the remote to which the branches were pushed.
	 */
	remote: string;
}

/**
 * All possible reasons for a commit to be rejected.
 *
 * This is used to display a message to the user when a commit fails.
 * @note - This reasons are in order of priority, from most to least important!
 */
export const REJECTTION_REASONS = [
	'workspaceMergeConflict',
	'cherryPickMergeConflict',
	'noEffectiveChanges',
	'worktreeFileMissingForObjectConversion',
	'fileToLargeOrBinary',
	'pathNotFoundInBaseTree',
	'unsupportedDirectoryEntry',
	'unsupportedTreeEntry',
	'missingDiffSpecAssociation'
] as const;

export type RejectionReason = (typeof REJECTTION_REASONS)[number];

export type CreateCommitOutcome = {
	newCommit: string | null;
	pathsToRejectedChanges: [RejectionReason, string][];
};

export const STACK_SERVICE = new InjectionToken<StackService>('StackService');

export class StackService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		backendApi: BackendApi,
		private dispatch: ThunkDispatch<any, any, UnknownAction>,
		private forgeFactory: DefaultForgeFactory,
		private uiState: UiState
	) {
		this.api = injectEndpoints(backendApi, uiState);
	}

	stacks(projectId: string) {
		return this.api.endpoints.stacks.useQuery(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectAll(stacks)
			}
		);
	}

	async fetchStacks(projectId: string) {
		return await this.api.endpoints.stacks.fetch(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectAll(stacks)
			}
		);
	}

	stackAt(projectId: string, index: number) {
		return this.api.endpoints.stacks.useQuery(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectNth(stacks, index)
			}
		);
	}

	stackById(projectId: string, id: string) {
		return this.api.endpoints.stacks.useQuery(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectById(stacks, id) ?? null
			}
		);
	}

	allStacks(projectId: string) {
		return this.api.endpoints.allStacks.useQuery(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectAll(stacks)
			}
		);
	}

	allStackAt(projectId: string, index: number) {
		return this.api.endpoints.allStacks.useQuery(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectNth(stacks, index)
			}
		);
	}

	allStackById(projectId: string, id: string) {
		return this.api.endpoints.allStacks.useQuery(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectById(stacks, id)
			}
		);
	}

	defaultBranch(projectId: string, stackId?: string) {
		if (!stackId) return null;
		return this.api.endpoints.stacks.useQuery(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectById(stacks, stackId)?.heads[0]?.name ?? null
			}
		);
	}

	stackInfo(projectId: string, stackId: string) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{ transform: ({ stackInfo }) => stackInfo }
		);
	}

	branchDetails(projectId: string, stackId: string | undefined, branchName?: string) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) => {
					return branchName
						? branchDetailsSelectors.selectById(branchDetails, branchName)
						: undefined;
				}
			}
		);
	}

	get newStack() {
		return this.api.endpoints.createStack.useMutation();
	}

	get newStackMutation() {
		return this.api.endpoints.createStack.mutate;
	}

	get updateStackOrder() {
		return this.api.endpoints.updateStackOrder.mutate;
	}

	branches(projectId: string, stackId?: string) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) => branchDetailsSelectors.selectAll(branchDetails)
			}
		);
	}

	branchAt(projectId: string, stackId: string | undefined, index: number) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ stackInfo }) => stackInfo.branchDetails[index]
			}
		);
	}

	/** Returns the parent of the branch specified by the provided name */
	branchParentByName(projectId: string, stackId: string | undefined, name: string) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ stackInfo, branchDetails }) => {
					const ids = stackInfo.branchDetails.map((branch) => branch.name);
					const currentId = ids.findIndex((id) => id === name);
					if (currentId === -1) return;

					const parentId = ids[currentId + 1];
					if (!parentId) return;

					return branchDetailsSelectors.selectById(branchDetails, parentId);
				}
			}
		);
	}
	/** Returns the child of the branch specified by the provided name */
	branchChildByName(projectId: string, stackId: string | undefined, name: string) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ stackInfo, branchDetails }) => {
					const ids = stackInfo.branchDetails.map((branch) => branch.name);
					const currentId = ids.findIndex((id) => id === name);
					if (currentId === -1) return;

					const childId = ids[currentId - 1];
					if (!childId) return;

					return branchDetailsSelectors.selectById(branchDetails, childId);
				}
			}
		);
	}

	branchByName(projectId: string, stackId: string | undefined, name: string) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{ transform: ({ branchDetails }) => branchDetailsSelectors.selectById(branchDetails, name) }
		);
	}

	commits(projectId: string, stackId: string | undefined, branchName: string) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)?.commits
			}
		);
	}

	fetchCommits(projectId: string, stackId: string | undefined, branchName: string) {
		return this.api.endpoints.stackDetails.fetch(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)?.commits
			}
		);
	}

	async fetchStackById(projectId: string, stackId: string) {
		return await this.api.endpoints.stacks.fetch(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectById(stacks, stackId)
			}
		);
	}

	async fetchBranches(projectId: string, stackId: string) {
		return await this.api.endpoints.stackDetails.fetch(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) => branchDetailsSelectors.selectAll(branchDetails)
			}
		);
	}

	commitAt(projectId: string, stackId: string | undefined, branchName: string, index: number) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)?.commits[index] ?? null
			}
		);
	}

	commitById(projectId: string, stackId: string | undefined, commitId: string) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ commits, upstreamCommits }) =>
					commitSelectors.selectById(commits, commitId) ??
					upstreamCommitSelectors.selectById(upstreamCommits, commitId)
			}
		);
	}

	commitsByIds(projectId: string, stackId: string | undefined, commitIds: string[]) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ commits, upstreamCommits }) => {
					const commitDetails = commitIds.map((id) => {
						return (
							commitSelectors.selectById(commits, id) ??
							upstreamCommitSelectors.selectById(upstreamCommits, id)
						);
					});
					return commitDetails.filter(isDefined);
				}
			}
		);
	}

	fetchCommitById(projectId: string, stackId: string, commitId: string) {
		return this.api.endpoints.stackDetails.fetch(
			{ projectId, stackId },
			{
				transform: ({ commits, upstreamCommits }) =>
					commitSelectors.selectById(commits, commitId) ??
					upstreamCommitSelectors.selectById(upstreamCommits, commitId)
			}
		);
	}

	fetchCommitsByIds(projectId: string, stackId: string, commitIds: string[]) {
		return this.api.endpoints.stackDetails.fetch(
			{ projectId, stackId },
			{
				transform: ({ commits, upstreamCommits }) => {
					const commitDetails = commitIds.map((id) => {
						return (
							commitSelectors.selectById(commits, id) ??
							upstreamCommitSelectors.selectById(upstreamCommits, id)
						);
					});
					return commitDetails.filter(isDefined);
				}
			}
		);
	}

	upstreamCommits(projectId: string, stackId: string | undefined, branchName: string) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)?.upstreamCommits
			}
		);
	}

	upstreamCommitAt(projectId: string, stackId: string, branchName: string, index: number) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)?.upstreamCommits[index] ??
					null
			}
		);
	}

	fetchUpstreamCommitById(projectId: string, stackId: string, commitId: string) {
		return this.api.endpoints.stackDetails.fetch(
			{ projectId, stackId },
			{
				transform: ({ upstreamCommits }) =>
					upstreamCommitSelectors.selectById(upstreamCommits, commitId)
			}
		);
	}

	get pushStack() {
		return this.api.endpoints.pushStack.useMutation({
			sideEffect: (_, args) => {
				// Timeout to accomodate eventual consistency.
				setTimeout(() => {
					this.forgeFactory.invalidate([
						invalidatesItem(ReduxTag.PullRequests, args.stackId),
						invalidatesItem(ReduxTag.Checks, args.stackId),
						invalidatesList(ReduxTag.PullRequests)
					]);
				}, 2000);
			},
			onError: (commandError: ReduxError) => {
				const { code, message } = commandError;
				const handled = surfaceStackError('push', code ?? '', message);
				if (!handled && code === 'errors.git.force_push_protection') {
					throw commandError;
				}
			},
			throwSlientError: true
		});
	}

	createCommit() {
		return this.api.endpoints.createCommit.useMutation();
	}

	get createCommitMutation() {
		return this.api.endpoints.createCommit.mutate;
	}

	filePathsChangedInCommits(projectId: string, commitIds: string[]) {
		const params = commitIds.map((commitId) => ({
			projectId,
			commitId
		}));
		return this.api.endpoints.commitDetails.useQueries(params, {
			transform: (results) => {
				return results.changes.ids;
			}
		});
	}

	commitChanges(projectId: string, commitId: string) {
		return this.api.endpoints.commitDetails.useQuery(
			{ projectId, commitId },
			{
				transform: (result) => ({
					changes: sortLikeFileTree(changesSelectors.selectAll(result.changes)),
					stats: result.stats,
					conflictEntries: result.conflictEntries
				})
			}
		);
	}

	fetchCommitChanges(projectId: string, commitId: string) {
		return this.api.endpoints.commitDetails.fetch(
			{ projectId, commitId },
			{
				transform: (result) => ({
					changes: changesSelectors.selectAll(result.changes),
					stats: result.stats,
					conflictEntries: result.conflictEntries
				})
			}
		);
	}

	commitChange(projectId: string, commitId: string, path: string) {
		return this.api.endpoints.commitDetails.useQuery(
			{ projectId, commitId },
			{ transform: (result) => changesSelectors.selectById(result.changes, path) }
		);
	}

	async commitChangesByPaths(projectId: string, commitId: string, paths: string[]) {
		const result = await this.api.endpoints.commitDetails.fetch(
			{ projectId, commitId },
			{ transform: (result) => selectChangesByPaths(result.changes, paths) }
		);
		return result || [];
	}

	commitDetails(projectId: string, commitId: string) {
		return this.api.endpoints.commitDetails.useQuery(
			{ projectId, commitId },
			{ transform: (result) => result.details }
		);
	}

	fetchCommitDetails(projectId: string, commitId: string) {
		return this.api.endpoints.commitDetails.fetch(
			{ projectId, commitId },
			{ transform: (result) => result.details }
		);
	}

	/**
	 * Gets the changes for a given branch.
	 * If the branch is part of a stack and if the stackId is provided, this will include only the changes up to the next branch in the stack.
	 * Otherwise, if stackId is not provided, this will include all changes as compared to the target branch
	 */
	branchChanges(args: { projectId: string; stackId?: string; branch: BranchRef }) {
		return this.api.endpoints.branchChanges.useQuery(
			{
				projectId: args.projectId,
				stackId: args.stackId,
				branch: args.branch
			},
			{
				transform: (result) => ({
					changes: changesSelectors.selectAll(result.changes),
					stats: result.stats
				})
			}
		);
	}

	branchChange(args: { projectId: string; stackId?: string; branch: BranchRef; path: string }) {
		return this.api.endpoints.branchChanges.useQuery(
			{
				projectId: args.projectId,
				stackId: args.stackId,
				branch: args.branch
			},
			{ transform: (result) => changesSelectors.selectById(result.changes, args.path) }
		);
	}

	async branchChangesByPaths(args: {
		projectId: string;
		stackId?: string;
		branch: BranchRef;
		paths: string[];
	}) {
		const result = await this.api.endpoints.branchChanges.fetch(
			{
				projectId: args.projectId,
				stackId: args.stackId,
				branch: args.branch
			},
			{ transform: (result) => selectChangesByPaths(result.changes, args.paths) }
		);
		return result || [];
	}

	get updateCommitMessage() {
		return this.api.endpoints.updateCommitMessage.useMutation();
	}

	get newBranch() {
		return this.api.endpoints.newBranch.useMutation();
	}

	async uncommit(args: {
		projectId: string;
		stackId: string;
		branchName: string;
		commitId: string;
	}) {
		const result = await this.api.endpoints.uncommit.mutate(args);
		const selection = this.uiState.lane(args.stackId).selection;
		if (args.commitId === selection.current?.commitId) {
			selection.set(undefined);
		}
		return result;
	}

	get insertBlankCommit() {
		return this.api.endpoints.insertBlankCommit.useMutation();
	}

	get unapply() {
		return this.api.endpoints.unapply.mutate;
	}

	get discardChanges() {
		return this.api.endpoints.discardChanges.mutate;
	}

	get moveChangesBetweenCommits() {
		return this.api.endpoints.moveChangesBetweenCommits.mutate;
	}

	get uncommitChanges() {
		return this.api.endpoints.uncommitChanges.mutate;
	}

	get stashIntoBranch() {
		return this.api.endpoints.stashIntoBranch.mutate;
	}

	get updateBranchPrNumber() {
		return this.api.endpoints.updateBranchPrNumber.mutate;
	}

	get updateBranchName() {
		return this.api.endpoints.updateBranchName.useMutation({
			sideEffect: (_, args) => {
				// Immediately update the selection and the exclusive action.
				const laneState = this.uiState.lane(args.laneId);
				const projectState = this.uiState.project(args.projectId);
				const exclusiveAction = projectState.exclusiveAction.current;
				const previousSelection = laneState.selection.current;

				if (previousSelection) {
					const updatedSelection = replaceBranchInStackSelection(
						previousSelection,
						args.branchName,
						args.newName
					);
					laneState.selection.set(updatedSelection);
				}

				if (exclusiveAction) {
					const updatedExclusiveAction = replaceBranchInExclusiveAction(
						exclusiveAction,
						args.branchName,
						args.newName
					);
					projectState.exclusiveAction.set(updatedExclusiveAction);
				}
			},
			onError: (_, args) => {
				const state = this.uiState.lane(args.laneId);
				state.selection.set({
					branchName: args.branchName
				});
			}
		});
	}

	get removeBranch() {
		return this.api.endpoints.removeBranch.useMutation();
	}

	get updateBranchDescription() {
		return this.api.endpoints.updateBranchDescription.useMutation();
	}

	get reorderStack() {
		return this.api.endpoints.reorderStack.mutate;
	}

	get moveCommit() {
		return this.api.endpoints.moveCommit.mutate;
	}

	get integrateUpstreamCommits() {
		return this.api.endpoints.integrateUpstreamCommits.useMutation();
	}

	initialIntegrationSteps(projectId: string, stackId: string | undefined, branchName: string) {
		return this.api.endpoints.getInitialIntegrationSteps.useQuery({
			projectId,
			stackId,
			branchName
		});
	}

	get integrateBranchWithSteps() {
		return this.api.endpoints.integrateBranchWithSteps.useMutation();
	}

	get createVirtualBranchFromBranch() {
		return this.api.endpoints.createVirtualBranchFromBranch.mutate;
	}

	get deleteLocalBranch() {
		return this.api.endpoints.deleteLocalBranch.mutate;
	}

	get squashCommits() {
		return this.api.endpoints.squashCommits.mutate;
	}

	get amendCommit() {
		return this.api.endpoints.amendCommit.useMutation();
	}

	get amendCommitMutation() {
		return this.api.endpoints.amendCommit.mutate;
	}

	/** Squash all the commits in a branch together */
	async squashAllCommits({
		projectId,
		stackId,
		branchName
	}: {
		projectId: string;
		stackId: string;
		branchName: string;
	}) {
		const allCommits = await this.api.endpoints.stackDetails.fetch(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)?.commits
			}
		);

		if (!allCommits) return;
		const localCommits = allCommits.filter((commit) => commit.state.type !== 'Integrated');

		if (localCommits.length <= 1) return;

		const targetCommit = localCommits.at(-1)!;
		const squashCommits = localCommits.slice(0, -1);

		await this.squashCommits({
			projectId,
			stackId,
			sourceCommitIds: squashCommits.map((commit) => commit.id),
			targetCommitId: targetCommit.id
		});
	}

	newBranchName(projectId: string) {
		return this.api.endpoints.newBranchName.useQuery({ projectId }, { forceRefetch: true });
	}

	async fetchNewBranchName(projectId: string) {
		return await this.api.endpoints.newBranchName.fetch({ projectId }, { forceRefetch: true });
	}

	isBranchConflicted(projectId: string, stackId: string, branchName: string) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) => {
					const branch = branchDetailsSelectors.selectById(branchDetails, branchName);
					return branch?.isConflicted ?? false;
				}
			}
		);
	}

	async normalizeBranchName(name: string) {
		return await this.api.endpoints.normalizeBranchName.fetch({ name }, { forceRefetch: true });
	}

	/**
	 * Note: This is specifically for looking up branches outside of
	 * a stacking context. You almost certainly want `stackDetails`
	 */
	unstackedBranchDetails(projectId: string, branchName: string, remote?: string) {
		return this.api.endpoints.unstackedBranchDetails.useQuery(
			{ projectId, branchName, remote },
			{ transform: (result) => result.branchDetails }
		);
	}

	unstackedCommits(projectId: string, branchName: string, remote?: string) {
		return this.api.endpoints.unstackedBranchDetails.useQuery(
			{ projectId, branchName, remote },
			{
				transform: (data) => commitSelectors.selectAll(data.commits)
			}
		);
	}

	async fetchUnstackedCommits(projectId: string, branchName: string, remote?: string) {
		return await this.api.endpoints.unstackedBranchDetails.fetch(
			{ projectId, branchName, remote },
			{
				transform: (data) => commitSelectors.selectAll(data.commits)
			}
		);
	}

	unstackedCommitById(projectId: string, branchName: string, commitId: string, remote?: string) {
		return this.api.endpoints.unstackedBranchDetails.useQuery(
			{ projectId, branchName, remote },
			{ transform: ({ commits }) => commitSelectors.selectById(commits, commitId) }
		);
	}

	async targetCommits(projectId: string, lastCommitId: string | undefined, pageSize: number) {
		return await this.api.endpoints.targetCommits.fetch(
			{ projectId, lastCommitId, pageSize },
			{
				forceRefetch: true,
				transform: (commits) => commitSelectors.selectAll(commits)
			}
		);
	}

	get splitBranch() {
		return this.api.endpoints.splitBranch.useMutation();
	}

	get splitBranchMutation() {
		return this.api.endpoints.splitBranch.mutate;
	}

	get splitBrancIntoDependentBranch() {
		return this.api.endpoints.splitBranchIntoDependentBranch.useMutation();
	}

	stackDetailsUpdateListener(projectId: string) {
		return this.api.endpoints.stackDetailsUpdate.useQuery({
			projectId
		});
	}

	invalidateStackDetailsUpdate(stackId: string) {
		this.dispatch(
			this.api.util.invalidateTags([
				invalidatesItem(ReduxTag.StackDetails, stackId),
				invalidatesList(ReduxTag.Stacks)
			])
		);
	}
	invalidateStacks() {
		this.dispatch(this.api.util.invalidateTags([invalidatesList(ReduxTag.Stacks)]));
	}

	templates(projectId: string, forgeName: string) {
		return this.api.endpoints.templates.useQuery({ projectId, forge: { name: forgeName } });
	}

	async template(projectId: string, forgeName: string, relativePath: string) {
		return await this.api.endpoints.template.fetch({
			relativePath,
			projectId,
			forge: { name: forgeName }
		});
	}

	get createReference() {
		return this.api.endpoints.createReference.useMutation();
	}
}

function transformStacksResponse(response: Stack[]) {
	// response.forEach((stack) => {
	// 	// To keep it simple, what's cast as `Stack` is actually `StackOpt`
	// 	// (as returned by the backend).
	// 	// So here we cast it back and stop any optional stack-id in its tracks
	// 	// until the code can actually cope with it.
	// 	const stackOpt = stack as StackOpt;
	// 	if (!stackOpt.id) {
	// 		throw new Error('BUG(opt-stack-id): cannot yet handle optional stack IDs');
	// 	}
	// });
	return stackAdapter.addMany(stackAdapter.getInitialState(), response);
}

function injectEndpoints(api: ClientState['backendApi'], uiState: UiState) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			stacks: build.query<EntityState<Stack, string>, { projectId: string }>({
				extraOptions: { command: 'stacks' },
				query: (args) => args,
				providesTags: [providesList(ReduxTag.Stacks)],
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasBackendExtra(lifecycleApi.extra)) {
						throw new Error('Redux dependency Tauri not found!');
					}
					// The `cacheDataLoaded` promise resolves when the result is first loaded.
					await lifecycleApi.cacheDataLoaded;
					const unsubscribe = lifecycleApi.extra.backend.listen(
						`project://${arg.projectId}/hunk-assignment-update`,
						() => {
							lifecycleApi.dispatch(api.util.invalidateTags([invalidatesList(ReduxTag.Stacks)]));
						}
					);
					// The `cacheEntryRemoved` promise resolves when the result is removed
					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				},
				transformResponse(response: Stack[], _, { projectId }) {
					// Clear the selection of stale stacks.
					updateStaleProjectState(
						uiState,
						projectId,
						// TODO(opt-stack-id): `s.id` might actually be optional once outside-of-workspace is a thing.
						response.map((s) => s.id).filter(isDefined)
					);

					return transformStacksResponse(response);
				}
			}),
			allStacks: build.query<EntityState<Stack, string>, { projectId: string }>({
				extraOptions: { command: 'stacks' },
				query: ({ projectId }) => ({ projectId, filter: 'All' }),
				providesTags: [providesList(ReduxTag.Stacks)],
				transformResponse: transformStacksResponse
			}),
			createStack: build.mutation<Stack, { projectId: string; branch: BranchParams }>({
				extraOptions: {
					command: 'create_virtual_branch',
					actionName: 'Create Stack'
				},
				query: (args) => args,
				invalidatesTags: (result, _error) => [
					invalidatesItem(ReduxTag.StackDetails, result?.id),
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.UpstreamIntegrationStatus),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			updateStackOrder: build.mutation<
				void,
				{ projectId: string; stacks: { id: string; order: number }[] }
			>({
				extraOptions: {
					command: 'update_stack_order',
					actionName: 'Update Stack Order'
				},
				query: (args) => args
				// This invalidation causes the order to jump back and forth
				// on save, and it's a bit unclear why. It's not important to
				// reload, however, so leaving it like this for now.
				// invalidatesTags: [invalidatesList(ReduxTag.Stacks)]
			}),
			stackDetails: build.query<
				{
					stackInfo: StackDetails;
					branchDetails: EntityState<BranchDetails, string>;
					commits: EntityState<Commit, string>;
					upstreamCommits: EntityState<UpstreamCommit, string>;
				},
				// TODO(single-branch): stackId is actually `stackId?` in the backend to be able to query details in single-branch mode.
				// 	  however, ideally all this goes away in favor of consuming `RefInfo` from the backend.
				{ projectId: string; stackId?: string }
			>({
				extraOptions: { command: 'stack_details' },
				query: (args) => args,
				providesTags: (_result, _error, { stackId }) => [
					...providesItem(ReduxTag.StackDetails, stackId || 'undefined')
				],
				transformResponse(response: StackDetails, _, { projectId, stackId }) {
					if (stackId) {
						updateStaleStackState(uiState, projectId, stackId, response);
					}

					const branchDetailsEntity = branchDetailsAdapter.addMany(
						branchDetailsAdapter.getInitialState(),
						response.branchDetails
					);

					// This is a list of all the commits accross all branches in the stack.
					// If you want to acces the commits of a specific branch, use the
					// `commits` property of the `BranchDetails` struct.
					const commitsEntity = commitAdapter.addMany(
						commitAdapter.getInitialState(),
						response.branchDetails.flatMap((branch) => branch.commits)
					);

					// This is a list of all the upstream commits across all the branches in the stack.
					// If you want to access the upstream commits of a specific branch, use the
					// `upstreamCommits` property of the `BranchDetails` struct.
					const upstreamCommitsEntity = upstreamCommitAdapter.addMany(
						upstreamCommitAdapter.getInitialState(),
						response.branchDetails.flatMap((branch) => branch.upstreamCommits)
					);

					return {
						stackInfo: response,
						branchDetails: branchDetailsEntity,
						commits: commitsEntity,
						upstreamCommits: upstreamCommitsEntity
					};
				}
			}),
			/**
			 * Note: This is specifically for looking up branches outside of
			 * a stacking context. You almost certainly want `stackDetails`
			 */
			unstackedBranchDetails: build.query<
				{
					branchDetails: BranchDetails;
					commits: EntityState<Commit, string>;
					upstreamCommits: EntityState<UpstreamCommit, string>;
				},
				{ projectId: string; branchName: string; remote?: string }
			>({
				extraOptions: { command: 'branch_details' },
				query: (args) => args,
				transformResponse(branchDetails: BranchDetails) {
					// This is a list of all the commits accross all branches in the stack.
					// If you want to acces the commits of a specific branch, use the
					// `commits` property of the `BranchDetails` struct.
					const commitsEntity = commitAdapter.addMany(
						commitAdapter.getInitialState(),
						branchDetails.commits
					);

					// This is a list of all the upstream commits across all the branches in the stack.
					// If you want to access the upstream commits of a specific branch, use the
					// `upstreamCommits` property of the `BranchDetails` struct.
					const upstreamCommitsEntity = upstreamCommitAdapter.addMany(
						upstreamCommitAdapter.getInitialState(),
						branchDetails.upstreamCommits
					);

					return {
						branchDetails,
						commits: commitsEntity,
						upstreamCommits: upstreamCommitsEntity
					};
				},
				providesTags: (_result, _error, { branchName }) => [
					...providesItem(ReduxTag.BranchDetails, branchName)
				]
			}),
			pushStack: build.mutation<
				BranchPushResult,
				{
					projectId: string;
					stackId?: string;
					withForce: boolean;
					skipForcePushProtection: boolean;
					branch: string;
					runHooks: boolean;
				}
			>({
				extraOptions: {
					command: 'push_stack',
					actionName: 'Push'
				},
				query: (args) => args,
				invalidatesTags: (result, _error, args) => {
					const invalidations = [
						invalidatesList(ReduxTag.Checks),
						invalidatesItem(ReduxTag.PullRequests, args.stackId),
						invalidatesItem(ReduxTag.StackDetails, args.stackId),
						invalidatesList(ReduxTag.BranchListing)
					];

					if (!result) return invalidations;

					const upstreamBranchNames = result.branchToRemote
						.map(([_, refname]) => getBranchNameFromRef(refname, result.remote))
						.filter(isDefined);
					if (upstreamBranchNames.length === 0) return invalidations;

					for (const upstreamBranchName of upstreamBranchNames) {
						invalidations.push(invalidatesItem(ReduxTag.Checks, upstreamBranchName));
					}

					return invalidations;
				}
			}),
			createCommit: build.mutation<
				CreateCommitOutcome,
				{ projectId: string } & CreateCommitRequest
			>({
				extraOptions: {
					command: 'create_commit_from_worktree_changes',
					actionName: 'Commit'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesList(ReduxTag.UpstreamIntegrationStatus),
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			commitDetails: build.query<
				{
					changes: EntityState<TreeChange, string>;
					details: Commit;
					stats: TreeStats;
					conflictEntries?: ConflictEntriesObj;
				},
				{ projectId: string; commitId: string }
			>({
				extraOptions: { command: 'commit_details' },
				query: (args) => args,
				providesTags: (_result, _error, { commitId }) => [
					...providesItem(ReduxTag.CommitChanges, commitId)
				],
				transformResponse(rsp: CommitDetails) {
					const changes = changesAdapter.addMany(changesAdapter.getInitialState(), rsp.changes);
					return {
						changes: changes,
						details: rsp.commit,
						stats: rsp.stats,
						conflictEntries: rsp.conflictEntries
							? new ConflictEntries(
									rsp.conflictEntries.ancestorEntries,
									rsp.conflictEntries.ourEntries,
									rsp.conflictEntries.theirEntries
								).toObj()
							: undefined
					};
				}
			}),
			branchChanges: build.query<
				{ changes: EntityState<TreeChange, string>; stats: TreeStats },
				{ projectId: string; stackId?: string; branch: BranchRef }
			>({
				extraOptions: { command: 'changes_in_branch' },
				query: (args) => args,
				providesTags: (_result, _error, { stackId }) =>
					stackId ? providesItem(ReduxTag.BranchChanges, stackId) : [],
				transformResponse(rsp: TreeChanges) {
					return {
						changes: changesAdapter.addMany(changesAdapter.getInitialState(), rsp.changes),
						stats: rsp.stats
					};
				}
			}),
			updateCommitMessage: build.mutation<
				string,
				{ projectId: string; stackId: string; commitId: string; message: string }
			>({
				extraOptions: {
					command: 'update_commit_message',
					actionName: 'Update Commit Message'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			newBranch: build.mutation<
				void,
				{ projectId: string; stackId: string; request: { targetPatch?: string; name: string } }
			>({
				extraOptions: {
					command: 'create_branch',
					actionName: 'Create Branch'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId),
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			uncommit: build.mutation<void, { projectId: string; stackId: string; commitId: string }>({
				extraOptions: {
					command: 'undo_commit',
					actionName: 'Uncommit'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			amendCommit: build.mutation<
				string /** Return value is the updated commit id. */,
				{
					projectId: string;
					stackId: string;
					commitId: string;
					worktreeChanges: DiffSpec[];
				}
			>({
				extraOptions: {
					command: 'amend_virtual_branch',
					actionName: 'Amend Commit'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesItem(ReduxTag.BranchChanges, args.stackId),
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			insertBlankCommit: build.mutation<
				void,
				{ projectId: string; stackId: string; commitId: string | undefined; offset: number }
			>({
				extraOptions: {
					command: 'insert_blank_commit',
					actionName: 'Insert Blank Commit'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			discardChanges: build.mutation<
				DiffSpec[],
				{ projectId: string; worktreeChanges: DiffSpec[] }
			>({
				extraOptions: {
					command: 'discard_worktree_changes',
					actionName: 'Discard Changes'
				},
				query: (args) => args,
				invalidatesTags: [invalidatesList(ReduxTag.WorktreeChanges)]
			}),
			moveChangesBetweenCommits: build.mutation<
				{ replacedCommits: [string, string][] },
				{
					projectId: string;
					changes: DiffSpec[];
					sourceCommitId: string;
					sourceStackId: string;
					destinationCommitId: string;
					destinationStackId: string;
				}
			>({
				extraOptions: {
					command: 'move_changes_between_commits',
					actionName: 'Move Changes Between Commits'
				},
				query: (args) => args,
				invalidatesTags(result, _error, arg) {
					const commitChangesTags = [arg.sourceCommitId, arg.destinationCommitId]
						.map((id) => result?.replacedCommits.find(([oldId]) => oldId === id)?.[1])
						.filter(isDefined)
						.map((id) => invalidatesItem(ReduxTag.CommitChanges, id));
					return [
						invalidatesItem(ReduxTag.StackDetails, arg.sourceStackId),
						invalidatesItem(ReduxTag.StackDetails, arg.destinationStackId),
						invalidatesList(ReduxTag.WorktreeChanges),
						...commitChangesTags
					];
				}
			}),
			uncommitChanges: build.mutation<
				{ replacedCommits: [string, string][] },
				{
					projectId: string;
					changes: DiffSpec[];
					commitId: string;
					stackId: string;
					assignTo?: string;
				}
			>({
				extraOptions: {
					command: 'uncommit_changes',
					actionName: 'Uncommit Changes'
				},
				query: (args) => args,
				invalidatesTags(_result, _error, arg) {
					return [
						invalidatesItem(ReduxTag.BranchChanges, arg.stackId),
						invalidatesItem(ReduxTag.StackDetails, arg.stackId),
						invalidatesList(ReduxTag.WorktreeChanges)
					];
				}
			}),
			stashIntoBranch: build.mutation<
				DiffSpec[],
				{ projectId: string; branchName: string; worktreeChanges: DiffSpec[] }
			>({
				extraOptions: {
					command: 'stash_into_branch',
					actionName: 'Stash Changes'
				},
				query: (args) => args,
				invalidatesTags: [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			unapply: build.mutation<void, { projectId: string; stackId: string }>({
				extraOptions: {
					command: 'unapply_stack',
					actionName: 'Unapply Stack'
				},
				query: (args) => args,
				invalidatesTags: () => [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			// TODO: Why is this not part of the regular update call?
			updateBranchPrNumber: build.mutation<
				void,
				{
					projectId: string;
					stackId: string;
					branchName: string;
					prNumber: number;
				}
			>({
				extraOptions: {
					command: 'update_branch_pr_number',
					actionName: 'Update Branch PR Number'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			updateBranchName: build.mutation<
				void,
				{
					projectId: string;
					stackId?: string;
					laneId: string;
					branchName: string;
					newName: string;
				}
			>({
				extraOptions: {
					command: 'update_branch_name',
					actionName: 'Update Branch Name'
				},
				query: (args) => args,
				invalidatesTags: (_r, _e, args) => [
					invalidatesList(ReduxTag.Stacks),
					invalidatesItem(ReduxTag.StackDetails, args.stackId),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			removeBranch: build.mutation<
				void,
				{
					projectId: string;
					stackId?: string;
					branchName: string;
				}
			>({
				extraOptions: {
					command: 'remove_branch',
					actionName: 'Remove Branch'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId),
					invalidatesList(ReduxTag.BranchListing),
					invalidatesList(ReduxTag.Stacks)
				]
			}),
			updateBranchDescription: build.mutation<
				void,
				{ projectId: string; stackId: string; branchName: string; description: string }
			>({
				extraOptions: {
					command: 'update_branch_description',
					actionName: 'Update Branch Description'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			reorderStack: build.mutation<
				void,
				{ projectId: string; stackId: string; stackOrder: StackOrder }
			>({
				extraOptions: {
					command: 'reorder_stack',
					actionName: 'Reorder Stack'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			moveCommit: build.mutation<
				MoveCommitIllegalAction | null,
				{ projectId: string; sourceStackId: string; commitId: string; targetStackId: string }
			>({
				extraOptions: {
					command: 'move_commit',
					actionName: 'Move Commit'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.WorktreeChanges), // Moving commits can cause conflicts
					invalidatesItem(ReduxTag.StackDetails, args.sourceStackId),
					invalidatesItem(ReduxTag.StackDetails, args.targetStackId)
				]
			}),
			integrateUpstreamCommits: build.mutation<
				void,
				{
					projectId: string;
					stackId: string;
					seriesName: string;
					strategy: SeriesIntegrationStrategy | undefined;
				}
			>({
				extraOptions: {
					command: 'integrate_upstream_commits',
					actionName: 'Integrate Upstream Commits'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			getInitialIntegrationSteps: build.query<
				InteractiveIntegrationStep[],
				{ projectId: string; stackId: string | undefined; branchName: string }
			>({
				extraOptions: { command: 'get_initial_integration_steps_for_branch' },
				query: (args) => args,
				providesTags: (_result, _error, { stackId, branchName }) =>
					providesItem(ReduxTag.IntegrationSteps, (stackId ?? '--no stack ID--') + branchName)
			}),
			integrateBranchWithSteps: build.mutation<
				void,
				{
					projectId: string;
					stackId: string;
					branchName: string;
					steps: InteractiveIntegrationStep[];
				}
			>({
				extraOptions: {
					command: 'integrate_branch_with_steps',
					actionName: 'Integrate Branch with Steps'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.IntegrationSteps, args.stackId + args.branchName),
					invalidatesItem(ReduxTag.StackDetails, args.stackId),
					invalidatesItem(ReduxTag.BranchDetails, args.branchName)
				]
			}),
			createVirtualBranchFromBranch: build.mutation<
				void,
				{ projectId: string; branch: string; remote?: string; prNumber?: number }
			>({
				extraOptions: {
					command: 'create_virtual_branch_from_branch',
					actionName: 'Create Virtual Branch From Branch'
				},
				query: (args) => args,
				invalidatesTags: [invalidatesList(ReduxTag.Stacks), invalidatesList(ReduxTag.BranchListing)]
			}),
			deleteLocalBranch: build.mutation<
				void,
				{ projectId: string; refname: string; givenName: string }
			>({
				extraOptions: {
					command: 'delete_local_branch',
					actionName: 'Delete Local Branch'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, { givenName: branchName }) => [
					invalidatesItem(ReduxTag.BranchDetails, branchName),
					providesList(ReduxTag.BranchListing)
				]
			}),
			squashCommits: build.mutation<
				void,
				{ projectId: string; stackId: string; sourceCommitIds: string[]; targetCommitId: string }
			>({
				extraOptions: {
					command: 'squash_commits',
					actionName: 'Squash Commits'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.WorktreeChanges), // Could cause conflicts
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			newBranchName: build.query<
				string,
				{
					projectId: string;
				}
			>({
				extraOptions: { command: 'canned_branch_name' },
				query: (args) => args
			}),
			normalizeBranchName: build.query<
				string,
				{
					name: string;
				}
			>({
				extraOptions: { command: 'normalize_branch_name' },
				query: (args) => args
			}),
			targetCommits: build.query<
				EntityState<Commit, string>,
				{
					projectId: string;
					lastCommitId: string | undefined;
					pageSize: number;
				}
			>({
				extraOptions: { command: 'target_commits' },
				query: (args) => args,
				transformResponse: (commits: Commit[]) =>
					commitAdapter.addMany(commitAdapter.getInitialState(), commits)
			}),
			splitBranch: build.mutation<
				{ replacedCommits: [string, string][] },
				{
					projectId: string;
					sourceStackId: string;
					sourceBranchName: string;
					newBranchName: string;
					fileChangesToSplitOff: string[];
				}
			>({
				extraOptions: {
					command: 'split_branch'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.sourceStackId),
					invalidatesItem(ReduxTag.BranchChanges, args.sourceStackId),
					invalidatesList(ReduxTag.Stacks)
				]
			}),
			splitBranchIntoDependentBranch: build.mutation<
				{ replacedCommits: [string, string][] },
				{
					projectId: string;
					sourceStackId: string;
					sourceBranchName: string;
					newBranchName: string;
					fileChangesToSplitOff: string[];
				}
			>({
				extraOptions: {
					command: 'split_branch_into_dependent_branch'
				},
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.sourceStackId),
					invalidatesItem(ReduxTag.BranchChanges, args.sourceStackId),
					invalidatesList(ReduxTag.Stacks)
				]
			}),
			createReference: build.mutation<
				void,
				{ projectId: string; stackId: string; request: CreateRefRequest }
			>({
				extraOptions: {
					command: 'create_reference',
					actionName: 'Create Reference'
				},
				query: (args) => {
					// TODO: Remove the stack ID from the request args.
					// The backend doesn't need it, but the frontend does to invalidate the right tags.
					// We should move away from using the stack ID as the cache key, an move towards some form of branch name instead.

					return { projectId: args.projectId, request: args.request };
				},
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			stackDetailsUpdate: build.query<void, { projectId: string }>({
				queryFn: () => ({ data: undefined }),
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasBackendExtra(lifecycleApi.extra)) {
						throw new Error('Stack details update endpoint requires Backend extra');
					}

					await lifecycleApi.cacheDataLoaded;
					const unsubscribe = lifecycleApi.extra.backend.listen<{ stackId: string }>(
						`project://${arg.projectId}/stack_details_update`,
						(event) => {
							lifecycleApi.dispatch(
								api.util.invalidateTags([
									invalidatesItem(ReduxTag.StackDetails, event.payload.stackId),
									invalidatesList(ReduxTag.Stacks),
									invalidatesList(ReduxTag.WorktreeChanges)
								])
							);
						}
					);

					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				}
			}),
			templates: build.query<string[], { projectId: string; forge: { name: string } }>({
				extraOptions: { command: 'pr_templates' },
				query: (args) => args
			}),
			template: build.query<
				string,
				{ projectId: string; forge: { name: string }; relativePath: string }
			>({
				extraOptions: { command: 'pr_template' },
				query: (args) => args
			})
		})
	});
}

const stackAdapter = createEntityAdapter<Stack, string>({
	selectId: (stack) => stack.id || stack.heads.at(0)?.name || stack.tip
});
const stackSelectors = { ...stackAdapter.getSelectors(), selectNth: createSelectNth<Stack>() };

const commitAdapter = createEntityAdapter<Commit, string>({
	selectId: (commit) => commit.id
});
const commitSelectors = { ...commitAdapter.getSelectors(), selectNth: createSelectNth<Commit>() };

const upstreamCommitAdapter = createEntityAdapter<UpstreamCommit, string>({
	selectId: (commit) => commit.id
});
const upstreamCommitSelectors = {
	...upstreamCommitAdapter.getSelectors(),
	selectNth: createSelectNth<UpstreamCommit>()
};

const changesAdapter = createEntityAdapter<TreeChange, string>({
	selectId: (change) => change.path
});

const changesSelectors = changesAdapter.getSelectors();

const selectChangesByPaths = createSelectByIds<TreeChange>();

const branchDetailsAdapter = createEntityAdapter<BranchDetails, string>({
	selectId: (branch) => branch.name
});

const branchDetailsSelectors = branchDetailsAdapter.getSelectors();
