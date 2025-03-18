import { showToast } from '$lib/notifications/toasts';
import { ClientState } from '$lib/state/clientState.svelte';
import { createSelectNth } from '$lib/state/customSelectors';
import { ReduxTag } from '$lib/state/tags';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { BranchPushResult } from '$lib/branches/branchController';
import type { Commit, StackBranch, UpstreamCommit } from '$lib/branches/v3';
import type { CommitKey } from '$lib/commits/commit';
import type { TreeChange } from '$lib/hunks/change';
import type { HunkHeader } from '$lib/hunks/hunk';
import type { Stack, StackInfo } from '$lib/stacks/stack';
import type { TauriCommandError } from '$lib/state/backendQuery';

type CreateBranchRequest = { name?: string; ownership?: string; order?: number };

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
export class StackService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(state: ClientState, posthog: PostHogWrapper) {
		this.api = injectEndpoints(state.backendApi, posthog);
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

	stackInfo(projectId: string, stackId: string) {
		return this.api.endpoints.stackInfo.useQuery({ projectId, stackId });
	}

	newStack() {
		return this.api.endpoints.createStack.useMutation();
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

	pushStack() {
		return this.api.endpoints.pushStack.useMutation();
	}

	createCommit() {
		return this.api.endpoints.createCommit.useMutation();
	}

	commitChanges(projectId: string, commitId: string) {
		const result = $derived(
			this.api.endpoints.commitChanges.useQuery(
				{ projectId, commitId },
				{ transform: (result) => changesSelectors.selectAll(result) }
			)
		);
		return result;
	}

	commitChange(projectId: string, commitId: string, path: string) {
		const result = $derived(
			this.api.endpoints.commitChanges.useQuery(
				{ projectId, commitId },
				{ transform: (result) => changesSelectors.selectById(result, path) }
			)
		);
		return result;
	}

	updateCommitMessage() {
		return this.api.endpoints.updateCommitMessage.useMutation();
	}

	newBranch() {
		return this.api.endpoints.newBranch.useMutation();
	}

	uncommit() {
		return this.api.endpoints.uncommit.useMutation();
	}

	insertBlankCommit() {
		return this.api.endpoints.insertBlankCommit.useMutation();
	}
}

function injectEndpoints(api: ClientState['backendApi'], posthog: PostHogWrapper) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			stacks: build.query<EntityState<Stack, string>, { projectId: string }>({
				query: ({ projectId }) => ({ command: 'stacks', params: { projectId } }),
				providesTags: [ReduxTag.Stacks],
				transformResponse(response: Stack[]) {
					return stackAdapter.addMany(stackAdapter.getInitialState(), response);
				}
			}),
			createStack: build.mutation<Stack, { projectId: string; branch: CreateBranchRequest }>({
				query: ({ projectId, branch }) => ({
					command: 'create_virtual_branch',
					params: { projectId, branch }
				}),
				invalidatesTags: (result, _error) => [
					ReduxTag.Stacks,
					{ type: ReduxTag.StackInfo, id: result?.id }
				]
			}),
			stackBranches: build.query<
				EntityState<StackBranch, string>,
				{ projectId: string; stackId: string }
			>({
				query: ({ projectId, stackId }) => ({
					command: 'stack_branches',
					params: { projectId, stackId }
				}),
				providesTags: [ReduxTag.StackBranches],
				transformResponse(response: StackBranch[]) {
					return branchAdapter.addMany(branchAdapter.getInitialState(), response);
				}
			}),
			stackInfo: build.query<StackInfo, { projectId: string; stackId: string }>({
				query: ({ projectId, stackId }) => ({
					command: 'stack_info',
					params: { projectId, stackId }
				}),
				providesTags: (_result, _error, { stackId }) => [{ type: ReduxTag.StackInfo, id: stackId }]
			}),
			localAndRemoteCommits: build.query<
				EntityState<Commit, string>,
				{ projectId: string; stackId: string; branchName: string }
			>({
				query: ({ projectId, stackId, branchName }) => ({
					command: 'stack_branch_local_and_remote_commits',
					params: { projectId, stackId, branchName }
				}),
				providesTags: [ReduxTag.Commits],
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
				providesTags: [ReduxTag.Commits],
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
					params: { projectId, branchId: stackId, withForce }
				}),
				invalidatesTags: (_result, _error, args) => [
					ReduxTag.StackBranches,
					ReduxTag.Commits,
					{ type: ReduxTag.StackInfo, id: args.stackId }
				],
				transformResponse(response: BranchPushResult) {
					posthog.capture('Push Successful');
					return response;
				},
				transformErrorResponse(commandError: TauriCommandError) {
					const { code, message } = commandError;
					posthog.capture('Push Failed', { error: { code, message } });
					surfaceStackError('push', code ?? '', message);
					return commandError;
				}
			}),
			createCommit: build.mutation<
				{ newCommit: string; pathsToRejectedChanges: string[] },
				{ projectId: string } & CreateCommitRequest
			>({
				query: ({ projectId, ...commitData }) => ({
					command: 'create_commit_from_worktree_changes',
					params: { projectId, ...commitData }
				}),
				invalidatesTags: (_result, _error, args) => [
					ReduxTag.StackBranches,
					ReduxTag.Commits,
					{ type: ReduxTag.StackInfo, id: args.stackId }
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
				providesTags: [ReduxTag.CommitChanges],
				transformResponse(changes: TreeChange[]) {
					return changesAdapter.addMany(changesAdapter.getInitialState(), changes);
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
				providesTags: [ReduxTag.BranchChanges],
				transformResponse(changes: TreeChange[]) {
					return changesAdapter.addMany(changesAdapter.getInitialState(), changes);
				}
			}),
			updateCommitMessage: build.mutation<
				void,
				{ projectId: string; branchId: string; commitOid: string; message: string }
			>({
				query: ({ projectId, branchId, commitOid, message }) => ({
					command: 'update_commit_message',
					params: { projectId, branchId, commitOid, message }
				}),
				invalidatesTags: (_result, _error, args) => [
					ReduxTag.StackBranches,
					{ type: ReduxTag.StackInfo, id: args.branchId }
				]
			}),
			newBranch: build.mutation<
				void,
				{ projectId: string; stackId: string; request: { targetPatch?: string; name: string } }
			>({
				query: ({ projectId, stackId, request: { targetPatch, name } }) => ({
					command: 'create_series',
					params: { projectId, stackId, request: { targetPatch, name } }
				}),
				invalidatesTags: (_result, _error, args) => [
					ReduxTag.StackBranches,
					{ type: ReduxTag.StackInfo, id: args.stackId }
				]
			}),
			uncommit: build.mutation<void, { projectId: string; stackId: string; commitId: string }>({
				query: ({ projectId, stackId: branchId, commitId: commitOid }) => ({
					command: 'undo_commit',
					params: { projectId, branchId, commitOid }
				}),
				invalidatesTags: (_result, _error, args) => [
					ReduxTag.StackBranches,
					ReduxTag.Commits,
					{ type: ReduxTag.StackInfo, id: args.stackId }
				]
			}),
			insertBlankCommit: build.mutation<
				void,
				{ projectId: string; branchId: string; commitOid: string; offset: number }
			>({
				query: ({ projectId, branchId, commitOid, offset }) => ({
					command: 'insert_blank_commit',
					params: { projectId, branchId, commitOid, offset }
				}),
				invalidatesTags: (_result, _error, args) => [
					ReduxTag.StackBranches,
					ReduxTag.Commits,
					{ type: ReduxTag.StackInfo, id: args.branchId }
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

const changesAdapter = createEntityAdapter<TreeChange, string>({
	selectId: (change) => change.path
});

const changesSelectors = changesAdapter.getSelectors();
