import { StackOrder } from '$lib/branches/branch';
import { showToast } from '$lib/notifications/toasts';
import { ClientState, type BackendApi } from '$lib/state/clientState.svelte';
import { createSelectNth } from '$lib/state/customSelectors';
import {
	invalidatesItem,
	invalidatesList,
	providesItem,
	providesItems,
	providesList,
	ReduxTag
} from '$lib/state/tags';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { Commit, StackBranch, UpstreamCommit } from '$lib/branches/v3';
import type { CommitKey } from '$lib/commits/commit';
import type { LocalFile } from '$lib/files/file';
import type { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
import type { TreeChange, TreeChanges } from '$lib/hunks/change';
import type { DiffSpec, Hunk, HunkHeader } from '$lib/hunks/hunk';
import type { BranchDetails, Stack, StackInfo } from '$lib/stacks/stack';
import type { TauriCommandError } from '$lib/state/backendQuery';
import type { User } from '$lib/user/user';

type BranchParams = {
	name?: string;
	ownership?: string;
	order?: number;
	allow_rebasing?: boolean;
	notes?: string;
	selected_for_changes?: boolean;
};

type CreateCommitRequest = {
	stackId: string;
	message: string;
	/** Undefined means that the backend will infer the parent to be the current head of stackBranchName */
	parentId: string | undefined;
	stackBranchName: string;
	worktreeChanges: {
		previousPathBytes?: number[];
		pathBytes: number[];
		hunkHeaders: HunkHeader[];
	}[];
};

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

export class StackService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		private readonly backendApi: BackendApi,
		private forgeFactory: DefaultForgeFactory,
		private readonly posthog: PostHogWrapper
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

	defaultBranch(projectId: string, stackId: string) {
		return this.api.endpoints.stackBranches.useQuery(
			{ projectId, stackId },
			{
				transform: (branches) =>
					branchSelectors
						.selectAll(branches)
						.filter((b) => !b.archived)
						.at(0)
			}
		);
	}

	stackInfo(projectId: string, stackId: string) {
		return this.api.endpoints.stackInfo.useQuery(
			{ projectId, stackId },
			{ transform: ({ stackInfo }) => stackInfo }
		);
	}

	branchDetails(projectId: string, stackId: string, branchName: string) {
		return this.api.endpoints.stackInfo.useQuery(
			{ projectId, stackId },
			{
				transform: ({ branchDetails }) =>
					branchDetailsSelectors.selectById(branchDetails, branchName)
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
		return this.api.endpoints.updateStack.useMutation();
	}

	get updateBranchOrder() {
		return this.api.endpoints.updateBranchOrder.useMutation();
	}

	branches(projectId: string, stackId: string) {
		return this.api.endpoints.stackBranches.useQuery(
			{ projectId, stackId },
			{
				transform: (branches) =>
					branchSelectors.selectAll(branches).filter((branch) => !branch.archived)
			}
		);
	}

	branchAt(projectId: string, stackId: string, index: number) {
		return this.api.endpoints.stackBranches.useQuery(
			{ projectId, stackId },
			{
				transform: (branches) => branchSelectors.selectNth(branches, index)
			}
		);
	}

	/** Returns the parent of the branch specified by the provided name */
	branchParentByName(projectId: string, stackId: string, name: string) {
		return this.api.endpoints.stackBranches.useQuery(
			{ projectId, stackId },
			{
				transform: (result) => {
					const ids = branchSelectors.selectIds(result);
					const currentId = ids.findIndex((id) => id === name);
					// If the branch is the bottom-most branch or not found, return
					if (currentId === -1 || currentId + 1 === ids.length) return;

					return branchSelectors.selectNth(result, currentId + 1);
				}
			}
		);
	}
	/** Returns the child of the branch specified by the provided name */
	branchChildByName(projectId: string, stackId: string, name: string) {
		return this.api.endpoints.stackBranches.useQuery(
			{ projectId, stackId },
			{
				transform: (result) => {
					const ids = branchSelectors.selectIds(result);
					const currentId = ids.findIndex((id) => id === name);
					// If the branch is the top-most branch or not found, return
					if (currentId === -1 || currentId === 0) return;

					return branchSelectors.selectNth(result, currentId - 1);
				}
			}
		);
	}

	branchByName(projectId: string, stackId: string, name: string) {
		return this.api.endpoints.stackBranches.useQuery(
			{ projectId, stackId },
			{ transform: (result) => branchSelectors.selectById(result, name) }
		);
	}

	commits(projectId: string, stackId: string, branchName: string) {
		const result = $derived(
			this.api.endpoints.localAndRemoteCommits.useQuery(
				{ projectId, stackId, branchName },
				{
					transform: (result) => commitSelectors.selectAll(result)
				}
			)
		);
		return result;
	}

	commitAt(projectId: string, stackId: string, branchName: string, index: number) {
		const result = $derived(
			this.api.endpoints.localAndRemoteCommits.useQuery(
				{ projectId, stackId, branchName },
				{
					transform: (result) => commitSelectors.selectNth(result, index) || null
				}
			)
		);
		return result;
	}

	commitById(projectId: string, commitKey: CommitKey) {
		const { stackId, branchName, commitId } = commitKey;
		const result = $derived(
			this.api.endpoints.localAndRemoteCommits.useQuery(
				{ projectId, stackId, branchName },
				{
					transform: (result) => {
						return commitSelectors.selectById(result, commitId);
					}
				}
			)
		);
		return result;
	}

	upstreamCommits(projectId: string, stackId: string, branchName: string) {
		const result = $derived(
			this.api.endpoints.upstreamCommits.useQuery(
				{ projectId, stackId, branchName },
				{
					transform: (result) => upstreamCommitSelectors.selectAll(result)
				}
			)
		);
		return result;
	}

	upstreamCommitAt(projectId: string, stackId: string, branchName: string, index: number) {
		const result = $derived(
			this.api.endpoints.upstreamCommits.useQuery(
				{ projectId, stackId, branchName },
				{
					transform: (result) => upstreamCommitSelectors.selectNth(result, index)
				}
			)
		);
		return result;
	}

	upstreamCommitById(projectId: string, commitKey: CommitKey) {
		const { stackId, branchName, commitId } = commitKey;
		const result = $derived(
			this.api.endpoints.upstreamCommits.useQuery(
				{ projectId, stackId, branchName },
				{ transform: (result) => upstreamCommitSelectors.selectById(result, commitId) }
			)
		);
		return result;
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
			}
		});
	}

	get createCommit() {
		return this.api.endpoints.createCommit.useMutation();
	}

	get createCommitLegacy() {
		return this.api.endpoints.createCommitLegacy.useMutation();
	}

	commitChanges(projectId: string, commitId: string) {
		const result = $derived(
			this.api.endpoints.commitChanges.useQuery(
				{ projectId, commitId },
				{ transform: (result) => commitChangesSelectors.selectAll(result) }
			)
		);
		return result;
	}

	commitChange(projectId: string, commitId: string, path: string) {
		const result = $derived(
			this.api.endpoints.commitChanges.useQuery(
				{ projectId, commitId },
				{ transform: (result) => commitChangesSelectors.selectById(result, path) }
			)
		);
		return result;
	}

	branchChanges(projectId: string, stackId: string, branchName: string) {
		return this.api.endpoints.branchChanges.useQuery(
			{ projectId, stackId, branchName },
			{ transform: (result) => branchChangesSelectors.selectAll(result) }
		);
	}

	branchChange(projectId: string, stackId: string, branchName: string, path: string) {
		return this.api.endpoints.branchChanges.useQuery(
			{ projectId, stackId, branchName },
			{ transform: (result) => branchChangesSelectors.selectById(result, path) }
		);
	}

	get updateCommitMessage() {
		return this.api.endpoints.updateCommitMessage.useMutation();
	}

	get newBranch() {
		return this.api.endpoints.newBranch.useMutation();
	}

	get uncommit() {
		return this.api.endpoints.uncommit.useMutation();
	}

	get insertBlankCommit() {
		return this.api.endpoints.insertBlankCommit.useMutation();
	}

	get unapply() {
		return this.api.endpoints.unapply.useMutation();
	}

	get publishBranch() {
		return this.api.endpoints.publishBranch.useMutation();
	}

	get amendCommit() {
		return this.api.endpoints.amendCommit.useMutation();
	}

	get discardChanges() {
		return this.api.endpoints.discardChanges.useMutation();
	}

	get updateBranchPrNumber() {
		return this.api.endpoints.updateBranchPrNumber.useMutation();
	}

	get updateBranchName() {
		return this.api.endpoints.updateBranchName.useMutation();
	}

	get removeBranch() {
		return this.api.endpoints.removeBranch.useMutation();
	}

	get updateBranchDescription() {
		return this.api.endpoints.updateBranchDescription.useMutation();
	}

	get reorderStack() {
		return this.api.endpoints.reorderStack.useMutation();
	}

	get reorderStackMutation() {
		return this.api.endpoints.reorderStack.mutate;
	}

	get moveCommit() {
		return this.api.endpoints.moveCommit.useMutation();
	}

	get moveCommitMutation() {
		return this.api.endpoints.moveCommit.mutate;
	}

	get integrateUpstreamCommits() {
		return this.api.endpoints.integrateUpstreamCommits.useMutation();
	}

	get legacyUnapplyLines() {
		return this.api.endpoints.legacyUnapplyLines.useMutation();
	}

	get legacyUnapplyHunk() {
		return this.api.endpoints.legacyUnapplyHunk.useMutation();
	}

	get legacyUnapplyFiles() {
		return this.api.endpoints.legacyUnapplyFiles.useMutation();
	}

	get legacyUpdateBranchOwnership() {
		return this.api.endpoints.legacyUpdateBranchOwnership.useMutation();
	}

	get legacyUpdateBranchOwnershipMutation() {
		return this.api.endpoints.legacyUpdateBranchOwnership.mutate;
	}

	get createVirtualBranchFromBranch() {
		return this.api.endpoints.createVirtualBranchFromBranch.useMutation();
	}

	get deleteLocalBranch() {
		return this.api.endpoints.deleteLocalBranch.useMutation();
	}

	get markResolved() {
		return this.api.endpoints.markResolved.useMutation();
	}

	get squashCommits() {
		return this.api.endpoints.squashCommits.useMutation();
	}

	get squashCommitsMutation() {
		return this.api.endpoints.squashCommits.mutate;
	}

	get amendCommitMutation() {
		return this.api.endpoints.amendCommit.mutate;
	}

	get moveCommitFileMutation() {
		return this.api.endpoints.moveCommitFile.mutate;
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
			createStack: build.mutation<Stack, { projectId: string; branch: BranchParams }>({
				query: ({ projectId, branch }) => ({
					command: 'create_virtual_branch',
					params: { projectId, branch },
					actionName: 'Create Stack'
				}),
				invalidatesTags: (result, _error) => [
					invalidatesItem(ReduxTag.StackInfo, result?.id),
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
					invalidatesItem(ReduxTag.StackInfo, result?.id),
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.UpstreamIntegrationStatus),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			updateBranchOrder: build.mutation<
				void,
				{ projectId: string; branches: { id: string; order: number }[] }
			>({
				query: ({ projectId, branches }) => ({
					command: 'update_branch_order',
					params: { projectId, branches },
					actionName: 'Update Branch Order'
				}),
				invalidatesTags: [invalidatesList(ReduxTag.Stacks)]
			}),
			stackBranches: build.query<
				EntityState<StackBranch, string>,
				{ projectId: string; stackId: string }
			>({
				query: ({ projectId, stackId }) => ({
					command: 'stack_branches',
					params: { projectId, stackId }
				}),
				providesTags: (_result, _error, { stackId }) => [
					...providesItem(ReduxTag.StackBranches, stackId)
				],
				transformResponse(response: StackBranch[]) {
					return branchAdapter.addMany(branchAdapter.getInitialState(), response);
				}
			}),
			stackInfo: build.query<
				{ stackInfo: StackInfo; branchDetails: EntityState<BranchDetails, string> },
				{ projectId: string; stackId: string }
			>({
				query: ({ projectId, stackId }) => ({
					command: 'stack_info',
					params: { projectId, stackId }
				}),
				providesTags: (_result, _error, { stackId }) => [
					...providesItem(ReduxTag.StackInfo, stackId)
				],
				transformResponse(response: StackInfo) {
					const branchDetilsEntity = branchDetailsAdapter.addMany(
						branchDetailsAdapter.getInitialState(),
						response.branchDetails
					);
					return {
						stackInfo: response,
						branchDetails: branchDetilsEntity
					};
				}
			}),
			localAndRemoteCommits: build.query<
				EntityState<Commit, string>,
				{ projectId: string; stackId: string; branchName: string }
			>({
				query: ({ projectId, stackId, branchName }) => ({
					command: 'stack_branch_local_and_remote_commits',
					params: { projectId, stackId, branchName }
				}),
				providesTags: (result, _, args) => {
					const stackCommitsTags = providesItem(ReduxTag.Commits, args.stackId);

					if (!result) return stackCommitsTags;

					const allCommits = commitSelectors.selectAll(result);
					const commitTags = providesItems(
						ReduxTag.Commit,
						allCommits.map((commit) => commit.id)
					);

					return [...stackCommitsTags, ...commitTags];
				},
				transformResponse(response: Commit[]) {
					return commitAdapter.addMany(commitAdapter.getInitialState(), response);
				}
			}),
			upstreamCommits: build.query<
				EntityState<UpstreamCommit, string>,
				{ projectId: string; stackId: string; branchName: string }
			>({
				query: ({ projectId, stackId, branchName }) => ({
					command: 'stack_branch_upstream_only_commits',
					params: { projectId, stackId, branchName }
				}),
				providesTags: [providesList(ReduxTag.Commits)],
				transformResponse(response: UpstreamCommit[]) {
					return upstreamCommitAdapter.addMany(upstreamCommitAdapter.getInitialState(), response);
				}
			}),
			pushStack: build.mutation<
				BranchPushResult,
				{ projectId: string; stackId: string; withForce: boolean }
			>({
				query: ({ projectId, stackId, withForce }) => ({
					command: 'push_stack',
					params: { projectId, stackId, withForce },
					actionName: 'Push'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.Checks),
					invalidatesItem(ReduxTag.StackBranches, args.stackId),
					invalidatesItem(ReduxTag.Commits, args.stackId),
					invalidatesItem(ReduxTag.PullRequests, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			createCommit: build.mutation<
				{ newCommit: string; pathsToRejectedChanges: string[] },
				{ projectId: string } & CreateCommitRequest
			>({
				query: ({ projectId, ...commitData }) => ({
					command: 'create_commit_from_worktree_changes',
					params: { projectId, ...commitData },
					actionName: 'Commit'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.UpstreamIntegrationStatus),
					invalidatesItem(ReduxTag.StackBranches, args.stackId),
					invalidatesItem(ReduxTag.Commits, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId)
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
					params: { projectId, stackId, message, ownership },
					actionName: 'Commit'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.UpstreamIntegrationStatus),
					invalidatesItem(ReduxTag.StackBranches, args.stackId),
					invalidatesItem(ReduxTag.Commits, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId)
				]
			}),
			commitChanges: build.query<
				EntityState<TreeChange, string>,
				{ projectId: string; commitId: string }
			>({
				query: ({ projectId, commitId }) => ({
					command: 'changes_in_commit',
					params: { projectId, commitId }
				}),
				providesTags: (_result, _error, { commitId }) => [
					...providesItem(ReduxTag.CommitChanges, commitId)
				],
				transformResponse(rsp: TreeChanges) {
					return commitChangesAdapter.addMany(commitChangesAdapter.getInitialState(), rsp.changes);
				}
			}),
			branchChanges: build.query<
				EntityState<TreeChange, string>,
				{ projectId: string; stackId: string; branchName: string }
			>({
				query: ({ projectId, stackId, branchName }) => ({
					command: 'changes_in_branch',
					params: { projectId, stackId, branchName }
				}),
				providesTags: (_result, _error, { stackId, branchName }) => [
					...providesItem(ReduxTag.BranchChanges, stackId + branchName)
				],
				transformResponse(rsp: TreeChanges) {
					return branchChangesAdapter.addMany(branchChangesAdapter.getInitialState(), rsp.changes);
				}
			}),
			updateCommitMessage: build.mutation<
				string,
				{ projectId: string; stackId: string; commitId: string; message: string }
			>({
				query: ({ projectId, stackId, commitId, message }) => ({
					command: 'update_commit_message',
					params: { projectId, stackId, commitOid: commitId, message },
					actionName: 'Update Commit Message'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackBranches, args.stackId),
					invalidatesItem(ReduxTag.Commit, args.commitId),
					invalidatesItem(ReduxTag.Commits, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId)
				]
			}),
			newBranch: build.mutation<
				void,
				{ projectId: string; stackId: string; request: { targetPatch?: string; name: string } }
			>({
				query: ({ projectId, stackId, request: { targetPatch, name } }) => ({
					command: 'create_branch',
					params: { projectId, stackId, request: { targetPatch, name } },
					actionName: 'Create Branch'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackBranches, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			uncommit: build.mutation<void, { projectId: string; stackId: string; commitId: string }>({
				query: ({ projectId, stackId, commitId: commitOid }) => ({
					command: 'undo_commit',
					params: { projectId, stackId, commitOid },
					actionName: 'Uncommit'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackBranches, args.stackId),
					invalidatesItem(ReduxTag.Commits, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId)
				]
			}),
			amendCommit: build.mutation<
				string /** Return value is the update commit value. */,
				{ projectId: string; stackId: string; commitId: string; worktreeChanges: DiffSpec[] }
			>({
				query: ({ projectId, stackId: stackId, commitId, worktreeChanges }) => ({
					command: 'amend_virtual_branch',
					params: { projectId, stackId, commitId, worktreeChanges },
					actionName: 'Amend Commit'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.Commits, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId)
				]
			}),
			insertBlankCommit: build.mutation<
				void,
				{ projectId: string; stackId: string; commitOid: string; offset: number }
			>({
				query: ({ projectId, stackId, commitOid, offset }) => ({
					command: 'insert_blank_commit',
					params: { projectId, stackId, commitOid, offset },
					actionName: 'Insert Blank Commit'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackBranches, args.stackId),
					invalidatesItem(ReduxTag.Commits, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId)
				]
			}),
			discardChanges: build.mutation<
				DiffSpec[],
				{ projectId: string; worktreeChanges: DiffSpec[] }
			>({
				query: ({ projectId, worktreeChanges }) => ({
					command: 'discard_worktree_changes',
					params: { projectId, worktreeChanges },
					actionName: 'Discard Changes'
				}),
				invalidatesTags: [invalidatesList(ReduxTag.WorktreeChanges)]
			}),
			unapply: build.mutation<void, { projectId: string; stackId: string }>({
				query: ({ projectId, stackId }) => ({
					command: 'save_and_unapply_virtual_branch',
					params: { projectId, stackId },
					actionName: 'Unapply Stack'
				}),
				invalidatesTags: () => [
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
					invalidatesItem(ReduxTag.StackBranches, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId)
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
					},
					actionName: 'Update Branch PR Number'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackBranches, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId),
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
					},
					actionName: 'Update Branch Name'
				}),
				invalidatesTags: (_r, _e, args) => [
					invalidatesList(ReduxTag.Stacks),
					invalidatesItem(ReduxTag.StackBranches, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId),
					invalidatesItem(ReduxTag.Commits, args.stackId),
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
					},
					actionName: 'Remove Branch'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackBranches, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			updateBranchDescription: build.mutation<
				void,
				{ projectId: string; stackId: string; branchName: string; description: string }
			>({
				query: ({ projectId, stackId, branchName, description }) => ({
					command: 'update_branch_description',
					params: { projectId, stackId, branchName, description },
					actionName: 'Update Branch Description'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.StackBranches, args.stackId),
					invalidatesList(ReduxTag.BranchListing)
				]
			}),
			reorderStack: build.mutation<
				void,
				{ projectId: string; stackId: string; stackOrder: StackOrder }
			>({
				query: ({ projectId, stackId, stackOrder }) => ({
					command: 'reorder_stack',
					params: { projectId, stackId, stackOrder },
					actionName: 'Reorder Stack'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.Commits, args.stackId),
					invalidatesItem(ReduxTag.StackBranches, args.stackId)
				]
			}),
			moveCommit: build.mutation<
				void,
				{ projectId: string; sourceStackId: string; commitOid: string; targetStackId: string }
			>({
				query: ({ projectId, sourceStackId, commitOid, targetStackId }) => ({
					command: 'move_commit',
					params: { projectId, sourceStackId, commitOid, targetStackId },
					actionName: 'Move Commit'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.Commits, args.sourceStackId),
					invalidatesItem(ReduxTag.StackInfo, args.sourceStackId),
					invalidatesItem(ReduxTag.Commits, args.targetStackId),
					invalidatesItem(ReduxTag.StackInfo, args.targetStackId)
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
					params: { projectId, stackId, seriesName, strategy },
					actionName: 'Integrate Upstream Commits'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.Commits, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId)
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
					},
					actionName: 'Legacy Unapply Lines'
				})
			}),
			legacyUnapplyHunk: build.mutation<void, { projectId: string; hunk: Hunk }>({
				query: ({ projectId, hunk }) => ({
					command: 'unapply_ownership',
					params: { projectId, ownership: `${hunk.filePath}:${hunk.id}-${hunk.hash}` },
					actionName: 'Legacy Unapply Hunk'
				})
			}),
			legacyUnapplyFiles: build.mutation<
				void,
				{ projectId: string; stackId: string; files: LocalFile[] }
			>({
				query: ({ projectId, stackId, files }) => ({
					command: 'reset_files',
					params: { projectId, stackId, files: files?.flatMap((f) => f.path) ?? [] },
					actionName: 'Legacy Unapply Files'
				})
			}),
			legacyUpdateBranchOwnership: build.mutation<
				void,
				{ projectId: string; stackId: string; ownership: string }
			>({
				query: ({ projectId, stackId, ownership }) => ({
					command: 'update_virtual_branch',
					params: { projectId, branch: { id: stackId, ownership } },
					actionName: 'Legacy Update Branch Ownership'
				})
			}),
			createVirtualBranchFromBranch: build.mutation<
				void,
				{ projectId: string; branch: string; remote?: string; prNumber?: number }
			>({
				query: ({ projectId, branch, remote, prNumber }) => ({
					command: 'create_virtual_branch_from_branch',
					params: { projectId, branch, remote, prNumber },
					actionName: 'Create Virtual Branch From Branch'
				}),
				invalidatesTags: [invalidatesList(ReduxTag.Stacks)]
			}),
			deleteLocalBranch: build.mutation<
				void,
				{ projectId: string; refname: string; givenName: string }
			>({
				query: ({ projectId, refname, givenName }) => ({
					command: 'delete_local_branch',
					params: { projectId, refname, givenName },
					actionName: 'Delete Local Branch'
				})
			}),
			markResolved: build.mutation<void, { projectId: string; path: string }>({
				query: ({ projectId, path }) => ({
					command: 'mark_resolved',
					params: { projectId, path },
					actionName: 'Mark File Resolved'
				})
			}),
			squashCommits: build.mutation<
				void,
				{ projectId: string; stackId: string; sourceCommitOids: string[]; targetCommitOid: string }
			>({
				query: ({ projectId, stackId, sourceCommitOids, targetCommitOid }) => ({
					command: 'squash_commits',
					params: { projectId, stackId, sourceCommitOids, targetCommitOid },
					actionName: 'Squash Commits'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.Commits, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId)
				]
			}),
			ammendCommit: build.mutation<
				void,
				{ projectId: string; stackId: string; commitId: string; worktreeChanges: DiffSpec[] }
			>({
				query: ({ projectId, stackId, commitId, worktreeChanges }) => ({
					command: 'amend_virtual_branch',
					params: { projectId, stackId, commitId, worktreeChanges },
					actionName: 'Amend Commit'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesItem(ReduxTag.Commits, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId)
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
					params: { projectId, stackId, fromCommitOid, toCommitOid, ownership },
					actionName: 'Move Commit File'
				}),
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.Commits, args.stackId),
					invalidatesItem(ReduxTag.StackInfo, args.stackId),
					invalidatesList(ReduxTag.BranchListing)
				]
			})
		})
	});
}

const stackAdapter = createEntityAdapter<Stack, string>({
	selectId: (stack) => stack.id
});
const stackSelectors = { ...stackAdapter.getSelectors(), selectNth: createSelectNth<Stack>() };

const branchAdapter = createEntityAdapter<StackBranch, string>({
	selectId: (branch) => branch.name
});
const branchSelectors = {
	...branchAdapter.getSelectors(),
	selectNth: createSelectNth<StackBranch>()
};

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

const commitChangesAdapter = createEntityAdapter<TreeChange, string>({
	selectId: (change) => change.path
});

const commitChangesSelectors = commitChangesAdapter.getSelectors();

const branchChangesAdapter = createEntityAdapter<TreeChange, string>({
	selectId: (change) => change.path
});

const branchChangesSelectors = branchChangesAdapter.getSelectors();

const branchDetailsAdapter = createEntityAdapter<BranchDetails, string>({
	selectId: (branch) => branch.name
});

const branchDetailsSelectors = branchDetailsAdapter.getSelectors();
