import { ClientState } from '$lib/state/clientState.svelte';
import { ReduxTag } from '$lib/state/tags';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import type { Commit, WorkspaceBranch } from '$lib/branches/v3';
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

	getStacks(projectId: string) {
		const { getStacks } = this.api.endpoints;
		const result = $derived(getStacks.useQuery({ projectId }));
		return result;
	}

	// eslint-disable-next-line @typescript-eslint/promise-function-async
	newStack(projectId: string, branch: CreateBranchRequest) {
		const { createStack } = this.api.endpoints;
		const result = $derived(createStack.useMutation({ projectId, branch }));
		return result;
	}

	getStackBranches(projectId: string, stackId: string) {
		const { getStackBranches } = this.api.endpoints;
		const result = $derived(
			getStackBranches.useQuery({ projectId, stackId }, { transform: branchSelectors.selectAll })
		);
		return result;
	}

	getBranchByIndex(projectId: string, stackId: string, index: number) {
		const { getStackBranches } = this.api.endpoints;
		const result = $derived(
			getStackBranches.useQuery(
				{ projectId, stackId },
				{ transform: (result) => branchSelectors.selectAll(result).at(index) }
			)
		);
		return result;
	}

	// eslint-disable-next-line @typescript-eslint/promise-function-async
	createCommit(projectId: string, request: CreateCommitRequest) {
		const result = $derived(this.api.endpoints.createCommit.useMutation({ projectId, ...request }));
		return result;
	}

	/**
	 * Does not support merge commits, i.e. 2 parent oldCommitId's yet
	 */
	commitChanges(projectId: string, oldCommitId: string, newCommitId: string) {
		const { commitChanges } = this.api.endpoints;
		const result = $derived(commitChanges.useQuery({ projectId, oldCommitId, newCommitId }));
		return result;
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getStacks: build.query<Stack[], { projectId: string }>({
				query: ({ projectId }) => ({ command: 'stacks', params: { projectId } }),
				providesTags: [ReduxTag.Stacks]
			}),
			createStack: build.mutation<Stack, { projectId: string; branch: CreateBranchRequest }>({
				query: ({ projectId, branch }) => ({
					command: 'create_virtual_branch',
					params: { projectId, branch }
				}),
				invalidatesTags: [ReduxTag.Stacks]
			}),
			getStackBranches: build.query<
				EntityState<WorkspaceBranch, string>,
				{ projectId: string; stackId: string }
			>({
				query: ({ projectId, stackId }) => ({
					command: 'stack_branches',
					params: { projectId, stackId }
				}),
				providesTags: [ReduxTag.StackBranches],
				transformResponse(response: WorkspaceBranch[]) {
					return branchAdapter.addMany(branchAdapter.getInitialState(), response);
				}
			}),
			createCommit: build.mutation<Commit, { projectId: string } & CreateCommitRequest>({
				query: ({ projectId, ...commitData }) => ({
					command: 'create_commit_from_worktree_changes',
					params: { projectId, ...commitData }
				}),
				invalidatesTags: [ReduxTag.StackBranches, ReduxTag.Commit]
			}),
			/**
			 * Does not support merge commits, i.e. 2 parent oldCommitId's yet
			 */
			commitChanges: build.query<
				TreeChange[],
				{ projectId: string; oldCommitId: string; newCommitId: string }
			>({
				query: ({ projectId, oldCommitId, newCommitId }) => ({
					command: 'commit_changes',
					params: { projectId, oldCommitId, newCommitId }
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

const branchAdapter = createEntityAdapter<WorkspaceBranch, WorkspaceBranch['name']>({
	selectId: (change) => change.name,
	sortComparer: (a, b) => a.name.localeCompare(b.name)
});

const branchSelectors = branchAdapter.getSelectors();
