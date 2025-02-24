import { ClientState } from '$lib/state/clientState.svelte';
import { createSelectNth } from '$lib/state/customSelectors';
import { ReduxTag } from '$lib/state/tags';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import type { Commit, StackBranch, UpstreamCommit } from '$lib/branches/v3';
import type { CommitKey } from '$lib/commits/commit';
import type { TreeChange } from '$lib/hunks/change';
import type { HunkHeader } from '$lib/hunks/hunk';
import type { Stack } from '$lib/stacks/stack';

type CreateBranchRequest = { name?: string; ownership?: string; order?: number };

type CreateCommitRequest = {
	stackId: string;
	message: string;
	parentId: string;
	worktreeChanges: {
		previousPathBytes?: number[];
		pathBytes: number[];
		hunkHeaders: HunkHeader[];
	}[];
};

export class StackService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(state: ClientState) {
		this.api = injectEndpoints(state.backendApi);
	}

	stacks(projectId: string) {
		const result = $derived(
			this.api.endpoints.stacks.useQuery(
				{ projectId },
				{
					transform: (stacks) => stackSelectors.selectAll(stacks)
				}
			)
		);
		return result;
	}

	stackAt(projectId: string, index: number) {
		const result = $derived(
			this.api.endpoints.stacks.useQuery(
				{ projectId },
				{
					transform: (stacks) => stackSelectors.selectNth(stacks, index)
				}
			)
		);
		return result;
	}

	// eslint-disable-next-line @typescript-eslint/promise-function-async
	newStack(projectId: string, branch: CreateBranchRequest) {
		const { createStack } = this.api.endpoints;
		const result = $derived(createStack.useMutation({ projectId, branch }));
		return result;
	}

	branches(projectId: string, stackId: string) {
		const result = $derived(
			this.api.endpoints.stackBranches.useQuery(
				{ projectId, stackId },
				{
					transform: (branches) =>
						branchSelectors.selectAll(branches).filter((branch) => !branch.archive)
				}
			)
		);
		return result;
	}

	branchAt(projectId: string, stackId: string, index: number) {
		const result = $derived(
			this.api.endpoints.stackBranches.useQuery(
				{ projectId, stackId },
				{
					transform: (branches) => branchSelectors.selectNth(branches, index)
				}
			)
		);
		return result;
	}

	branchByName(projectId: string, stackId: string, name: string) {
		const result = $derived(
			this.api.endpoints.stackBranches.useQuery(
				{ projectId, stackId },
				{ transform: (result) => branchSelectors.selectById(result, name) }
			)
		);
		return result;
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
				{ transform: (result) => commitSelectors.selectById(result, commitId) }
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

	// eslint-disable-next-line @typescript-eslint/promise-function-async
	createCommit(projectId: string, request: CreateCommitRequest) {
		const result = $derived(this.api.endpoints.createCommit.useMutation({ projectId, ...request }));
		return result;
	}

	commitChanges(projectId: string, commitId: string) {
		const result = $derived(this.api.endpoints.commitChanges.useQuery({ projectId, commitId }));
		return result;
	}

	// eslint-disable-next-line @typescript-eslint/promise-function-async
	updateCommitMessage(projectId: string, branchId: string, commitOid: string, message: string) {
		const result = $derived(
			this.api.endpoints.updateCommitMessage.useMutation({
				projectId,
				branchId,
				commitOid,
				message
			})
		);
		return result;
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
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
				invalidatesTags: [ReduxTag.Stacks]
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
			createCommit: build.mutation<Commit, { projectId: string } & CreateCommitRequest>({
				query: ({ projectId, ...commitData }) => ({
					command: 'create_commit_from_worktree_changes',
					params: { projectId, ...commitData }
				}),
				invalidatesTags: [ReduxTag.StackBranches, ReduxTag.Commit]
			}),
			commitChanges: build.query<TreeChange[], { projectId: string; commitId: string }>({
				query: ({ projectId, commitId }) => ({
					command: 'changes_in_commit',
					params: { projectId, commitId }
				}),
				providesTags: [ReduxTag.CommitChanges]
			}),
			updateCommitMessage: build.mutation<
				void,
				{ projectId: string; branchId: string; commitOid: string; message: string }
			>({
				query: ({ projectId, branchId, commitOid, message }) => ({
					command: 'update_commit_message',
					params: { projectId, branchId, commitOid, message }
				}),
				invalidatesTags: [ReduxTag.StackBranches]
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
