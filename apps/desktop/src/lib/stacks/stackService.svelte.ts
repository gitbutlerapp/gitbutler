import { StackOrder } from '$lib/branches/branch';
import { ConflictEntries, type ConflictEntriesObj } from '$lib/files/conflicts';
import { showToast } from '$lib/notifications/toasts';
import { hasTauriExtra } from '$lib/state/backendQuery';
import { ClientState, type BackendApi } from '$lib/state/clientState.svelte';
import { createSelectByIds, createSelectNth } from '$lib/state/customSelectors';
import {
	invalidatesItem,
	invalidatesList,
	providesItem,
	providesList,
	ReduxTag
} from '$lib/state/tags';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import {
	createEntityAdapter,
	type EntityState,
	type ThunkDispatch,
	type UnknownAction
} from '@reduxjs/toolkit';
import type { TauriCommandError } from '$lib/backend/ipc';
import type { Commit, CommitDetails, UpstreamCommit } from '$lib/branches/v3';
import type { CommitKey } from '$lib/commits/commit';
import type { LocalFile } from '$lib/files/file';
import type { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
import type { TreeChange, TreeChanges } from '$lib/hunks/change';
import type { DiffSpec, Hunk } from '$lib/hunks/hunk';
import type { BranchDetails, Stack, StackDetails } from '$lib/stacks/stack';
import type { PropertiesFn } from '$lib/state/customHooks.svelte';
import type { UiState } from '$lib/state/uiState.svelte';
import type { User } from '$lib/user/user';

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
			['errors.git.authentication']: 'an authentication failure'
		},
		defaultInfo: 'an unforeseen error'
	}
};

function surfaceStackError(action: StackAction, errorCode: string, errorMessage: string): boolean {
	const reason = ERROR_INFO[action].codeInfo[errorCode] ?? ERROR_INFO[action].defaultInfo;
	const title = ERROR_INFO[action].title;
	switch (action) {
		case 'push': {
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

export type CommitIdOrChangeId = { CommitId: string } | { ChangeId: string };
export type SeriesIntegrationStrategy = 'merge' | 'rebase' | 'hardreset';

export interface BranchPushResult {
	refname: string;
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

export class StackService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		backendApi: BackendApi,
		private dispatch: ThunkDispatch<any, any, UnknownAction>,
		private forgeFactory: DefaultForgeFactory,
		private uiState: UiState
	) {
		this.api = injectEndpoints(backendApi);
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
				transform: (stacks) => stackSelectors.selectById(stacks, id)
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

	defaultBranch(projectId: string, stackId: string) {
		return this.api.endpoints.stacks.useQuery(
			{ projectId },
			{
				transform: (stacks) => stackSelectors.selectById(stacks, stackId)?.heads[0]?.name
			}
		);
	}

	stackInfo(projectId: string, stackId: string) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{ transform: ({ stackInfo }) => stackInfo }
		);
	}

	branchDetails(projectId: string, stackId: string, branchName?: string) {
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

	get updateStack() {
		return this.api.endpoints.updateStack.mutate;
	}

	get updateStackOrder() {
		return this.api.endpoints.updateStackOrder.mutate;
	}

	branches(projectId: string, stackId: string) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) => branchDetailsSelectors.selectAll(branchDetails)
			}
		);
	}

	branchAt(projectId: string, stackId: string, index: number) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ stackInfo }) => stackInfo.branchDetails[index]
			}
		);
	}

	/** Returns the parent of the branch specified by the provided name */
	branchParentByName(projectId: string, stackId: string, name: string) {
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
	branchChildByName(projectId: string, stackId: string, name: string) {
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

	branchByName(projectId: string, stackId: string, name: string) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{ transform: ({ branchDetails }) => branchDetailsSelectors.selectById(branchDetails, name) }
		);
	}

	commits(projectId: string, stackId: string, branchName: string) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)?.commits
			}
		);
	}

	fetchCommits(projectId: string, stackId: string, branchName: string) {
		return this.api.endpoints.stackDetails.fetch(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)?.commits
			}
		);
	}

	commitAt(projectId: string, stackId: string, branchName: string, index: number) {
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)?.commits[index] ?? null
			}
		);
	}

	commitById(projectId: string, commitKey: CommitKey) {
		const { stackId, commitId } = commitKey;
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ commits }) => commitSelectors.selectById(commits, commitId)
			}
		);
	}

	fetchCommitById(projectId: string, stackId: string, commitId: string) {
		return this.api.endpoints.stackDetails.fetch(
			{ projectId, stackId },
			{
				transform: ({ commits }) => commitSelectors.selectById(commits, commitId)
			}
		);
	}

	upstreamCommits(projectId: string, stackId: string, branchName: string) {
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

	upstreamCommitById(projectId: string, commitKey: CommitKey) {
		const { stackId, commitId } = commitKey;
		return this.api.endpoints.stackDetails.useQuery(
			{ projectId, stackId },
			{
				transform: ({ upstreamCommits }) =>
					upstreamCommitSelectors.selectById(upstreamCommits, commitId)
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
			onError: (commandError: TauriCommandError) => {
				const { code, message } = commandError;
				surfaceStackError('push', code ?? '', message);
			},
			throwSlientError: true
		});
	}

	createCommit(args: { propertiesFn?: PropertiesFn }) {
		const propertiesFn = args.propertiesFn;
		return this.api.endpoints.createCommit.useMutation({ propertiesFn });
	}

	get createCommitMutation() {
		return this.api.endpoints.createCommit.mutate;
	}

	get createCommitLegacy() {
		return this.api.endpoints.createCommitLegacy.mutate;
	}

	commitChanges(projectId: string, commitId: string) {
		return this.api.endpoints.commitDetails.useQuery(
			{ projectId, commitId },
			{
				transform: (result) => ({
					changes: changesSelectors.selectAll(result.changes),
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
		if (result.error) {
			throw result.error;
		}
		return result.data || [];
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
	branchChanges(args: {
		projectId: string;
		stackId?: string;
		branchName: string;
		remote?: string;
	}) {
		return this.api.endpoints.branchChanges.useQuery(
			{
				projectId: args.projectId,
				stackId: args.stackId,
				branchName: args.branchName,
				remote: args.remote
			},
			{ transform: (result) => changesSelectors.selectAll(result) }
		);
	}

	branchChange(args: {
		projectId: string;
		stackId?: string;
		branchName: string;
		remote?: string;
		path: string;
	}) {
		return this.api.endpoints.branchChanges.useQuery(
			{
				projectId: args.projectId,
				stackId: args.stackId,
				branchName: args.branchName,
				remote: args.remote
			},
			{ transform: (result) => changesSelectors.selectById(result, args.path) }
		);
	}

	async branchChangesByPaths(args: {
		projectId: string;
		stackId?: string;
		branchName: string;
		remote?: string;
		paths: string[];
	}) {
		const result = await this.api.endpoints.branchChanges.fetch(
			{
				projectId: args.projectId,
				stackId: args.stackId,
				branchName: args.branchName,
				remote: args.remote
			},
			{ transform: (result) => selectChangesByPaths(result, args.paths) }
		);
		if (result.error) {
			throw result.error;
		}
		return result.data || [];
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
		const selection = this.uiState.stack(args.stackId).selection;
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

	get publishBranch() {
		return this.api.endpoints.publishBranch.useMutation();
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
			preEffect: (args) => {
				const state = this.uiState.stack(args.stackId);
				state.selection.set(undefined);
			},
			sideEffect: (_, args) => {
				const state = this.uiState.stack(args.stackId);
				state.selection.set({
					branchName: args.newName
				});
			},
			onError: (_, args) => {
				const state = this.uiState.stack(args.stackId);
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

	get legacyUnapplyLines() {
		return this.api.endpoints.legacyUnapplyLines.mutate;
	}

	get legacyUnapplyHunk() {
		return this.api.endpoints.legacyUnapplyHunk.mutate;
	}

	get legacyUnapplyFiles() {
		return this.api.endpoints.legacyUnapplyFiles.mutate;
	}

	get legacyUpdateBranchOwnership() {
		return this.api.endpoints.legacyUpdateBranchOwnership.mutate;
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

	get moveCommitFileMutation() {
		return this.api.endpoints.moveCommitFile.mutate;
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

		if (!allCommits?.data) return;
		const localCommits = allCommits.data.filter((commit) => commit.state.type !== 'Integrated');

		if (localCommits.length <= 1) return;

		const targetCommit = localCommits.at(-1)!;
		const squashCommits = localCommits.slice(0, -1);
		// API squashes them in the order they are given, so we must reverse the list.
		squashCommits.reverse();

		await this.squashCommits({
			projectId,
			stackId,
			sourceCommitOids: squashCommits.map((commit) => commit.id),
			targetCommitOid: targetCommit.id
		});
	}

	async newBranchName(projectId: string) {
		return await this.api.endpoints.newBranchName.fetch({ projectId }, { forceRefetch: true });
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

	get enterEditMode() {
		return this.api.endpoints.enterEditMode.mutate;
	}

	get abortEditAndReturnToWorkspace() {
		return this.api.endpoints.abortEditAndReturnToWorkspace.mutate;
	}

	get saveEditAndReturnToWorkspace() {
		return this.api.endpoints.saveEditAndReturnToWorkspace.mutate;
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
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			stacks: build.query<EntityState<Stack, string>, { projectId: string }>({
				query: ({ projectId }) => ({ command: 'stacks', params: { projectId } }),
				providesTags: [providesList(ReduxTag.Stacks)],
				transformResponse(response: Stack[]) {
					return stackAdapter.addMany(stackAdapter.getInitialState(), response);
				}
			}),
			allStacks: build.query<EntityState<Stack, string>, { projectId: string }>({
				query: ({ projectId }) => ({ command: 'stacks', params: { projectId, filter: 'All' } }),
				providesTags: [providesList(ReduxTag.Stacks)],
				transformResponse(response: Stack[]) {
					return stackAdapter.addMany(stackAdapter.getInitialState(), response);
				}
			}),
			createStack: build.mutation<Stack, { projectId: string; branch: BranchParams }>({
				query: ({ projectId, branch }) => ({
					command: 'create_virtual_branch',
					params: { projectId, branch },
					actionName: 'Create Stack'
				}),
				invalidatesTags: (result, _error) => [
					invalidatesItem(ReduxTag.StackDetails, result?.id),
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.UpstreamIntegrationStatus),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			updateStack: build.mutation<
				Stack,
				{ projectId: string; branch: BranchParams & { id: string } }
			>({
				query: ({ projectId, branch }) => ({
					command: 'update_virtual_branch',
					params: { projectId, branch },
					actionName: 'Update Stack'
				}),
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
				query: ({ projectId, stacks }) => ({
					command: 'update_stack_order',
					params: { projectId, stacks },
					actionName: 'Update Stack Order'
				})
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
				{ projectId: string; stackId: string }
			>({
				query: ({ projectId, stackId }) => ({
					command: 'stack_details',
					params: { projectId, stackId }
				}),
				providesTags: (_result, _error, { stackId }) => [
					...providesItem(ReduxTag.StackDetails, stackId)
				],
				transformResponse(response: StackDetails) {
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
				query: ({ projectId, branchName, remote }) => ({
					command: 'branch_details',
					params: { projectId, branchName, remote }
				}),
				extraOptions: { actionName: 'Unstacked Branch Details' },
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
					stackId: string;
					withForce: boolean;
					/** if set, it will push up to this branch (inclusive) */
					branch?: string | undefined;
				}
			>({
				query: ({ projectId, stackId, withForce, branch }) => ({
					command: 'push_stack',
					params: { projectId, stackId, withForce, branch }
				}),
				extraOptions: { actionName: 'Push' },
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.Checks),
					invalidatesItem(ReduxTag.PullRequests, args.stackId),
					invalidatesItem(ReduxTag.StackDetails, args.stackId),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			createCommit: build.mutation<
				CreateCommitOutcome,
				{ projectId: string } & CreateCommitRequest
			>({
				query: ({ projectId, ...commitData }) => ({
					command: 'create_commit_from_worktree_changes',
					params: { projectId, ...commitData }
				}),
				extraOptions: { actionName: 'Commit' },
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesList(ReduxTag.UpstreamIntegrationStatus),
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			createCommitLegacy: build.mutation<
				undefined,
				{
					projectId: string;
					stackId: string;
					message: string;
					ownership: string | undefined;
				}
			>({
				query: ({ projectId, stackId, message, ownership }) => ({
					command: 'commit_virtual_branch',
					params: { projectId, stackId, message, ownership }
				}),
				extraOptions: { actionName: 'Commit' },
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.UpstreamIntegrationStatus),
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			commitDetails: build.query<
				{
					changes: EntityState<TreeChange, string>;
					details: Commit;
					conflictEntries?: ConflictEntriesObj;
				},
				{ projectId: string; commitId: string }
			>({
				query: ({ projectId, commitId }) => ({
					command: 'commit_details',
					params: { projectId, commitId }
				}),
				providesTags: (_result, _error, { commitId }) => [
					...providesItem(ReduxTag.CommitChanges, commitId)
				],
				transformResponse(rsp: CommitDetails) {
					const changes = changesAdapter.addMany(changesAdapter.getInitialState(), rsp.changes);
					return {
						changes: changes,
						details: rsp.commit,
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
				EntityState<TreeChange, string>,
				{ projectId: string; stackId?: string; branchName: string; remote?: string }
			>({
				query: ({ projectId, stackId, branchName, remote }) => ({
					command: 'changes_in_branch',
					params: { projectId, stackId, branchName, remote }
				}),
				providesTags: (_result, _error, { stackId }) =>
					stackId ? providesItem(ReduxTag.BranchChanges, stackId) : [],
				transformResponse(rsp: TreeChanges) {
					return changesAdapter.addMany(changesAdapter.getInitialState(), rsp.changes);
				}
			}),
			updateCommitMessage: build.mutation<
				string,
				{ projectId: string; stackId: string; commitId: string; message: string }
			>({
				query: ({ projectId, stackId, commitId, message }) => ({
					command: 'update_commit_message',
					params: { projectId, stackId, commitOid: commitId, message }
				}),
				extraOptions: { actionName: 'Update Commit Message' },
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			newBranch: build.mutation<
				void,
				{ projectId: string; stackId: string; request: { targetPatch?: string; name: string } }
			>({
				query: ({ projectId, stackId, request: { targetPatch, name } }) => ({
					command: 'create_branch',
					params: { projectId, stackId, request: { targetPatch, name } }
				}),
				extraOptions: { actionName: 'Create Branch' },
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			uncommit: build.mutation<void, { projectId: string; stackId: string; commitId: string }>({
				query: ({ projectId, stackId, commitId: commitOid }) => ({
					command: 'undo_commit',
					params: { projectId, stackId, commitOid }
				}),
				extraOptions: { actionName: 'Uncommit' },
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
				query: ({ projectId, stackId, commitId, worktreeChanges }) => ({
					command: 'amend_virtual_branch',
					params: { projectId, stackId, commitId, worktreeChanges }
				}),
				extraOptions: { actionName: 'Amend Commit' },
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesItem(ReduxTag.BranchChanges, args.stackId),
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			insertBlankCommit: build.mutation<
				void,
				{ projectId: string; stackId: string; commitOid: string | undefined; offset: number }
			>({
				query: ({ projectId, stackId, commitOid, offset }) => ({
					command: 'insert_blank_commit',
					params: { projectId, stackId, commitOid, offset }
				}),
				extraOptions: { actionName: 'Insert Blank Commit' },
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			discardChanges: build.mutation<
				DiffSpec[],
				{ projectId: string; worktreeChanges: DiffSpec[] }
			>({
				query: ({ projectId, worktreeChanges }) => ({
					command: 'discard_worktree_changes',
					params: { projectId, worktreeChanges }
				}),
				extraOptions: { actionName: 'Discard Changes' },
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
				query: ({
					projectId,
					changes,
					sourceCommitId,
					sourceStackId,
					destinationCommitId,
					destinationStackId
				}) => ({
					command: 'move_changes_between_commits',
					params: {
						projectId,
						changes,
						sourceCommitId,
						sourceStackId,
						destinationCommitId,
						destinationStackId
					}
				}),
				extraOptions: { actionName: 'Move Changes Between Commits' },
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
				query: ({ projectId, changes, commitId, stackId, assignTo }) => ({
					command: 'uncommit_changes',
					params: {
						projectId,
						changes,
						commitId,
						stackId,
						assignTo
					}
				}),
				extraOptions: { actionName: 'Uncommit Changes' },
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
				query: ({ projectId, branchName, worktreeChanges }) => ({
					command: 'stash_into_branch',
					params: { projectId, branchName, worktreeChanges },
					actionName: 'Stash Changes'
				}),
				extraOptions: { actionName: 'Stash Changes' },
				invalidatesTags: [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			unapply: build.mutation<void, { projectId: string; stackId: string }>({
				query: ({ projectId, stackId }) => ({
					command: 'unapply_stack',
					params: { projectId, stackId }
				}),
				extraOptions: { actionName: 'Unapply Stack' },
				invalidatesTags: () => [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			publishBranch: build.mutation<
				string,
				{ projectId: string; stackId: string; user: User; topBranch: string }
			>({
				query: ({ projectId, stackId, user, topBranch }) => ({
					command: 'push_stack_to_review',
					params: { projectId, stackId, user, topBranch }
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
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
				query: ({ projectId, stackId, branchName, prNumber }) => ({
					command: 'update_branch_pr_number',
					params: {
						projectId,
						stackId,
						branchName,
						prNumber
					}
				}),
				extraOptions: { actionName: 'Update Branch PR Number' },
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			updateBranchName: build.mutation<
				void,
				{
					projectId: string;
					stackId: string;
					branchName: string;
					newName: string;
				}
			>({
				query: ({ projectId, stackId, branchName, newName }) => ({
					command: 'update_branch_name',
					params: {
						projectId,
						stackId,
						branchName,
						newName
					}
				}),
				extraOptions: { actionName: 'Update Branch Name' },
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
					stackId: string;
					branchName: string;
				}
			>({
				query: ({ projectId, stackId, branchName }) => ({
					command: 'remove_branch',
					params: {
						projectId,
						stackId,
						branchName
					}
				}),
				extraOptions: { actionName: 'Remove Branch' },
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			updateBranchDescription: build.mutation<
				void,
				{ projectId: string; stackId: string; branchName: string; description: string }
			>({
				query: ({ projectId, stackId, branchName, description }) => ({
					command: 'update_branch_description',
					params: { projectId, stackId, branchName, description }
				}),
				extraOptions: { actionName: 'Update Branch Description' },
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			reorderStack: build.mutation<
				void,
				{ projectId: string; stackId: string; stackOrder: StackOrder }
			>({
				query: ({ projectId, stackId, stackOrder }) => ({
					command: 'reorder_stack',
					params: { projectId, stackId, stackOrder }
				}),
				extraOptions: { actionName: 'Reorder Stack' },
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			moveCommit: build.mutation<
				void,
				{ projectId: string; sourceStackId: string; commitOid: string; targetStackId: string }
			>({
				query: ({ projectId, sourceStackId, commitOid, targetStackId }) => ({
					command: 'move_commit',
					params: { projectId, sourceStackId, commitOid, targetStackId }
				}),
				extraOptions: { actionName: 'Move Commit' },
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
				query: ({ projectId, stackId, seriesName, strategy }) => ({
					command: 'integrate_upstream_commits',
					params: { projectId, stackId, seriesName, strategy }
				}),
				extraOptions: { actionName: 'Integrate Upstream Commits' },
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			legacyUnapplyLines: build.mutation<
				void,
				{ projectId: string; hunk: Hunk; linesToUnapply: { old?: number; new?: number }[] }
			>({
				query: ({ projectId, hunk, linesToUnapply }) => ({
					command: 'unapply_lines',
					params: {
						projectId,
						ownership: `${hunk.filePath}:${hunk.id}-${hunk.hash}`,
						lines: { [hunk.id]: linesToUnapply }
					}
				}),
				extraOptions: { actionName: 'Legacy Unapply Lines' }
			}),
			legacyUnapplyHunk: build.mutation<void, { projectId: string; hunk: Hunk }>({
				query: ({ projectId, hunk }) => ({
					command: 'unapply_ownership',
					params: { projectId, ownership: `${hunk.filePath}:${hunk.id}-${hunk.hash}` }
				}),
				extraOptions: { actionName: 'Legacy Unapply Hunk' }
			}),
			legacyUnapplyFiles: build.mutation<
				void,
				{ projectId: string; stackId: string; files: LocalFile[] }
			>({
				query: ({ projectId, stackId, files }) => ({
					command: 'reset_files',
					params: { projectId, stackId, files: files?.flatMap((f) => f.path) ?? [] }
				}),
				extraOptions: { actionName: 'Legacy Unapply Files' }
			}),
			legacyUpdateBranchOwnership: build.mutation<
				void,
				{ projectId: string; stackId: string; ownership: string }
			>({
				query: ({ projectId, stackId, ownership }) => ({
					command: 'update_virtual_branch',
					params: { projectId, branch: { id: stackId, ownership } }
				}),
				extraOptions: { actionName: 'Legacy Update Branch Ownership' }
			}),
			createVirtualBranchFromBranch: build.mutation<
				void,
				{ projectId: string; branch: string; remote?: string; prNumber?: number }
			>({
				query: ({ projectId, branch, remote, prNumber }) => ({
					command: 'create_virtual_branch_from_branch',
					params: { projectId, branch, remote, prNumber }
				}),
				extraOptions: { actionName: 'Create Virtual Branch From Branch' },
				invalidatesTags: [invalidatesList(ReduxTag.Stacks), invalidatesList(ReduxTag.BranchListing)]
			}),
			deleteLocalBranch: build.mutation<
				void,
				{ projectId: string; refname: string; givenName: string }
			>({
				query: ({ projectId, refname, givenName }) => ({
					command: 'delete_local_branch',
					params: { projectId, refname, givenName }
				}),
				extraOptions: { actionName: 'Delete Local Branch' },
				invalidatesTags: (_result, _error, { givenName: branchName }) => [
					invalidatesItem(ReduxTag.BranchDetails, branchName)
				]
			}),
			squashCommits: build.mutation<
				void,
				{ projectId: string; stackId: string; sourceCommitOids: string[]; targetCommitOid: string }
			>({
				query: ({ projectId, stackId, sourceCommitOids, targetCommitOid }) => ({
					command: 'squash_commits',
					params: { projectId, stackId, sourceCommitOids, targetCommitOid }
				}),
				extraOptions: { actionName: 'Squash Commits' },
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.WorktreeChanges), // Could cause conflicts
					invalidatesItem(ReduxTag.StackDetails, args.stackId)
				]
			}),
			moveCommitFile: build.mutation<
				void,
				{
					projectId: string;
					stackId: string;
					fromCommitOid: string;
					toCommitOid: string;
					ownership: string;
				}
			>({
				query: ({ projectId, stackId, fromCommitOid, toCommitOid, ownership }) => ({
					command: 'move_commit_file',
					params: { projectId, stackId, fromCommitOid, toCommitOid, ownership }
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.WorktreeChanges), // Could cause conflicts
					invalidatesItem(ReduxTag.StackDetails, args.stackId),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			newBranchName: build.query<
				string,
				{
					projectId: string;
				}
			>({
				query: ({ projectId }) => ({
					command: 'canned_branch_name',
					params: { projectId }
				})
			}),
			normalizeBranchName: build.query<
				string,
				{
					name: string;
				}
			>({
				query: ({ name }) => ({
					command: 'normalize_branch_name',
					params: { name }
				})
			}),
			targetCommits: build.query<
				EntityState<Commit, string>,
				{
					projectId: string;
					lastCommitId: string | undefined;
					pageSize: number;
				}
			>({
				query: (params) => ({
					command: 'target_commits',
					params
				}),
				transformResponse: (commits: Commit[]) =>
					commitAdapter.addMany(commitAdapter.getInitialState(), commits)
			}),
			enterEditMode: build.mutation<
				void,
				{ projectId: string; commitOid: string; stackId: string }
			>({
				query: (params) => ({
					command: 'enter_edit_mode',
					params
				})
			}),
			abortEditAndReturnToWorkspace: build.mutation<void, { projectId: string }>({
				query: (params) => ({
					command: 'abort_edit_and_return_to_workspace',
					params
				})
			}),
			saveEditAndReturnToWorkspace: build.mutation<void, { projectId: string }>({
				query: (params) => ({
					command: 'save_edit_and_return_to_workspace',
					params
				}),
				invalidatesTags: [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesList(ReduxTag.StackDetails)
				]
			}),
			stackDetailsUpdate: build.query<void, { projectId: string }>({
				queryFn: () => ({ data: undefined }),
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasTauriExtra(lifecycleApi.extra)) {
						throw new Error('Stack details update endpoint requires Tauri extra');
					}

					await lifecycleApi.cacheDataLoaded;
					const unsubscribe = lifecycleApi.extra.tauri.listen<{ stackId: string }>(
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
			})
		})
	});
}

const stackAdapter = createEntityAdapter<Stack, string>({
	selectId: (stack) => stack.id
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
