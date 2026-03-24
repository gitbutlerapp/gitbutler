import { ConflictEntries, type ConflictEntriesObj } from "$lib/files/conflicts";
import { createSelectByIds, createSelectNth } from "$lib/state/customSelectors";
import {
	invalidatesItem,
	invalidatesList,
	providesItem,
	providesList,
	ReduxTag,
} from "$lib/state/tags";
import { isDefined } from "@gitbutler/ui/utils/typeguards";
import { createEntityAdapter, type EntityState } from "@reduxjs/toolkit";
import type { StackOrder } from "$lib/branches/branch";
import type { Commit, CommitDetails, UpstreamCommit } from "$lib/branches/v3";
import type { MoveCommitIllegalAction } from "$lib/commits/commit";
import type { TreeChange, TreeChanges, TreeStats } from "$lib/hunks/change";
import type { DiffSpec } from "$lib/hunks/hunk";
import type {
	BranchDetails,
	Stack,
	StackDetails,
	CreateRefRequest,
	InteractiveIntegrationStep,
	CreateBranchFromBranchOutcome,
	MoveBranchResult,
	GerritPushFlag,
} from "$lib/stacks/stack";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";
import type { RejectionReason } from "$lib/state/uiState.svelte";
import type { HunkAssignment } from "@gitbutler/core/api";

export type { RejectionReason };

export type BranchParams = {
	name?: string;
	order?: number;
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

export type SeriesIntegrationStrategy = {
	type: "merge" | "rebase";
};

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
	"workspaceMergeConflict",
	"cherryPickMergeConflict",
	"noEffectiveChanges",
	"worktreeFileMissingForObjectConversion",
	"fileToLargeOrBinary",
	"pathNotFoundInBaseTree",
	"unsupportedDirectoryEntry",
	"unsupportedTreeEntry",
	"missingDiffSpecAssociation",
] as const;

type ReplacedCommit = [string, string];

type BackendCreateCommitOutcome = {
	newCommit?: string | null;
	pathsToRejectedChanges: [RejectionReason, string][];
	replacedCommits: Record<string, string>;
};

export type CreateCommitOutcome = {
	newCommit: string | null;
	pathsToRejectedChanges: [RejectionReason, string][];
	commitMapping: ReplacedCommit[];
};

type BackendCommitRewordResult = {
	newCommit: string;
	replacedCommits: Record<string, string>;
};

type BackendCommitInsertBlankResult = {
	newCommit: string;
	replacedCommits: Record<string, string>;
};

type BackendMoveChangesResult = {
	replacedCommits: Record<string, string>;
};

export type RelativeTo =
	| {
			type: "commit";
			subject: string;
	  }
	| {
			type: "reference";
			subject: string;
	  };

export function normalizeCreateCommitOutcome(
	response: BackendCreateCommitOutcome,
): CreateCommitOutcome {
	return {
		newCommit: response.newCommit ?? null,
		pathsToRejectedChanges: response.pathsToRejectedChanges,
		commitMapping: Object.entries(response.replacedCommits),
	};
}

export function transformStacksResponse(response: Stack[]) {
	return stackAdapter.addMany(stackAdapter.getInitialState(), response);
}

export function toCommitCreatePlacement(args: CreateCommitRequest): {
	relativeTo: RelativeTo;
	side: "above" | "below";
} {
	if (args.parentId) {
		return {
			relativeTo: {
				type: "commit",
				subject: args.parentId,
			},
			side: "above",
		};
	}

	return {
		relativeTo: {
			type: "reference",
			subject: args.stackBranchName.startsWith("refs/")
				? args.stackBranchName
				: `refs/heads/${args.stackBranchName}`,
		},
		side: "below",
	};
}

// Entity adapters and selectors

export const stackAdapter = createEntityAdapter<Stack, string>({
	selectId: (stack) => stack.id || stack.heads.at(0)?.name || stack.tip,
});
export const stackSelectors = {
	...stackAdapter.getSelectors(),
	selectNth: createSelectNth<Stack>(),
};

export const commitAdapter = createEntityAdapter<Commit, string>({
	selectId: (commit) => commit.id,
});
export const commitSelectors = {
	...commitAdapter.getSelectors(),
	selectNth: createSelectNth<Commit>(),
};

export const upstreamCommitAdapter = createEntityAdapter<UpstreamCommit, string>({
	selectId: (commit) => commit.id,
});
export const upstreamCommitSelectors = {
	...upstreamCommitAdapter.getSelectors(),
	selectNth: createSelectNth<UpstreamCommit>(),
};

export const changesAdapter = createEntityAdapter<TreeChange, string>({
	selectId: (change) => change.path,
});

export const changesSelectors = changesAdapter.getSelectors();

export const selectChangesByPaths = createSelectByIds<TreeChange>();

export const branchDetailsAdapter = createEntityAdapter<BranchDetails, string>({
	selectId: (branch) => branch.name,
});

export const branchDetailsSelectors = branchDetailsAdapter.getSelectors();

export function buildStackEndpoints(build: BackendEndpointBuilder) {
	return {
		stacks: build.query<EntityState<Stack, string>, { projectId: string; all?: boolean }>({
			extraOptions: { command: "stacks" },
			query: (args) => {
				const filter = args.all ? "All" : undefined;
				return { projectId: args.projectId, filter };
			},
			providesTags: [providesList(ReduxTag.Stacks)],
			transformResponse(response: Stack[]) {
				return transformStacksResponse(response);
			},
		}),
		createStack: build.mutation<Stack, { projectId: string; branch: BranchParams }>({
			extraOptions: {
				command: "create_virtual_branch",
				actionName: "Create Stack",
			},
			query: (args) => args,
			invalidatesTags: (result, _error) => [
				invalidatesItem(ReduxTag.StackDetails, result?.id || "undefined"),
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.UpstreamIntegrationStatus),
				invalidatesList(ReduxTag.BranchListing),
			],
		}),
		updateStackOrder: build.mutation<
			void,
			{ projectId: string; stacks: { id: string; order: number }[] }
		>({
			extraOptions: {
				command: "update_stack_order",
				actionName: "Update Stack Order",
			},
			query: (args) => args,
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
			extraOptions: { command: "stack_details" },
			query: (args) => args,
			providesTags: (_result, _error, { stackId }) => [
				...providesItem(ReduxTag.StackDetails, stackId || "undefined"),
			],
			transformResponse(response: StackDetails) {
				const branchDetailsEntity = branchDetailsAdapter.addMany(
					branchDetailsAdapter.getInitialState(),
					response.branchDetails,
				);

				// This is a list of all the commits across all branches in the stack.
				// If you want to access the commits of a specific branch, use the
				// `commits` property of the `BranchDetails` struct.
				const commitsEntity = commitAdapter.addMany(
					commitAdapter.getInitialState(),
					response.branchDetails.flatMap((branch) => branch.commits),
				);

				// This is a list of all the upstream commits across all the branches in the stack.
				// If you want to access the upstream commits of a specific branch, use the
				// `upstreamCommits` property of the `BranchDetails` struct.
				const upstreamCommitsEntity = upstreamCommitAdapter.addMany(
					upstreamCommitAdapter.getInitialState(),
					response.branchDetails.flatMap((branch) => branch.upstreamCommits),
				);

				return {
					stackInfo: response,
					branchDetails: branchDetailsEntity,
					commits: commitsEntity,
					upstreamCommits: upstreamCommitsEntity,
				};
			},
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
			extraOptions: { command: "branch_details" },
			query: (args) => args,
			transformResponse(branchDetails: BranchDetails) {
				// This is a list of all the commits across all branches in the stack.
				// If you want to access the commits of a specific branch, use the
				// `commits` property of the `BranchDetails` struct.
				const commitsEntity = commitAdapter.addMany(
					commitAdapter.getInitialState(),
					branchDetails.commits,
				);

				// This is a list of all the upstream commits across all the branches in the stack.
				// If you want to access the upstream commits of a specific branch, use the
				// `upstreamCommits` property of the `BranchDetails` struct.
				const upstreamCommitsEntity = upstreamCommitAdapter.addMany(
					upstreamCommitAdapter.getInitialState(),
					branchDetails.upstreamCommits,
				);

				return {
					branchDetails,
					commits: commitsEntity,
					upstreamCommits: upstreamCommitsEntity,
				};
			},
			providesTags: (_result, _error, { branchName }) => [
				...providesItem(ReduxTag.BranchDetails, branchName),
			],
		}),
		pushStack: build.mutation<
			BranchPushResult,
			{
				projectId: string;
				stackId: string;
				withForce: boolean;
				skipForcePushProtection: boolean;
				branch: string;
				runHooks: boolean;
				pushOpts: GerritPushFlag[];
			}
		>({
			extraOptions: {
				command: "push_stack",
				actionName: "Push",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesItem(ReduxTag.StackDetails, args.stackId), // Is this still needed?
				invalidatesList(ReduxTag.BranchListing),
			],
		}),
		legacyCreateCommit: build.mutation<
			CreateCommitOutcome,
			{ projectId: string } & CreateCommitRequest
		>({
			extraOptions: {
				command: "create_commit_from_worktree_changes",
				actionName: "Commit",
			},
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.UpstreamIntegrationStatus),
				invalidatesList(ReduxTag.HeadSha),
			],
		}),
		commitCreate: build.mutation<CreateCommitOutcome, { projectId: string } & CreateCommitRequest>({
			extraOptions: {
				command: "commit_create",
				actionName: "Commit",
			},
			query: (args) => {
				const { relativeTo, side } = toCommitCreatePlacement(args);
				return {
					projectId: args.projectId,
					relativeTo,
					side,
					changes: args.worktreeChanges,
					message: args.message,
				};
			},
			transformResponse: normalizeCreateCommitOutcome,
			invalidatesTags: [
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.UpstreamIntegrationStatus),
				invalidatesList(ReduxTag.HeadSha),
			],
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
			keepUnusedDataFor: 60, // Keep for 1 minute after last use
			extraOptions: { command: "commit_details_with_line_stats" },
			query: (args) => args,
			providesTags: (_result, _error, { commitId }) => [
				...providesItem(ReduxTag.CommitChanges, commitId),
			],
			transformResponse(rsp: CommitDetails) {
				const changes = changesAdapter.addMany(changesAdapter.getInitialState(), rsp.changes);
				const stats = rsp.stats;
				return {
					changes: changes,
					details: rsp.commit,
					stats,
					conflictEntries: rsp.conflictEntries
						? new ConflictEntries(
								rsp.conflictEntries.ancestorEntries,
								rsp.conflictEntries.ourEntries,
								rsp.conflictEntries.theirEntries,
							).toObj()
						: undefined,
				};
			},
		}),
		branchChanges: build.query<
			{ changes: EntityState<TreeChange, string>; stats: TreeStats },
			{ projectId: string; stackId?: string; branch: string }
		>({
			extraOptions: { command: "branch_diff" },
			query: (args) => args,
			providesTags: (_result, _error, { stackId }) =>
				stackId ? providesItem(ReduxTag.BranchChanges, stackId) : [],
			transformResponse(rsp: TreeChanges) {
				return {
					changes: changesAdapter.addMany(changesAdapter.getInitialState(), rsp.changes),
					stats: rsp.stats,
				};
			},
		}),
		legacyUpdateCommitMessage: build.mutation<
			string,
			{ projectId: string; stackId: string; commitId: string; message: string }
		>({
			extraOptions: {
				command: "update_commit_message",
				actionName: "Update Commit Message",
			},
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.HeadSha)],
		}),
		updateCommitMessage: build.mutation<
			string,
			{ projectId: string; stackId: string; commitId: string; message: string }
		>({
			extraOptions: {
				command: "commit_reword",
				actionName: "Update Commit Message",
			},
			query: (args) => args,
			transformResponse: (response: BackendCommitRewordResult) => response.newCommit,
			invalidatesTags: (_result, _error, { stackId }) => [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesItem(ReduxTag.StackDetails, stackId),
			],
		}),
		newBranch: build.mutation<
			void,
			{ projectId: string; stackId: string; request: { targetPatch?: string; name: string } }
		>({
			extraOptions: {
				command: "create_branch",
				actionName: "Create Branch",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesItem(ReduxTag.StackDetails, args.stackId),
				invalidatesList(ReduxTag.BranchListing),
			],
		}),
		uncommit: build.mutation<void, { projectId: string; stackId: string; commitId: string }>({
			extraOptions: {
				command: "undo_commit",
				actionName: "Uncommit",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesItem(ReduxTag.BranchChanges, args.stackId),
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.HeadSha),
			],
		}),
		legacyAmendCommit: build.mutation<
			string /** Return value is the updated commit id. */,
			{
				projectId: string;
				stackId: string;
				commitId: string;
				worktreeChanges: DiffSpec[];
			}
		>({
			extraOptions: {
				command: "amend_virtual_branch",
				actionName: "Amend Commit",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesItem(ReduxTag.BranchChanges, args.stackId),
				invalidatesList(ReduxTag.HeadSha),
			],
		}),
		commitAmend: build.mutation<
			string /** Return value is the updated commit id. */,
			{
				projectId: string;
				commitId: string;
				worktreeChanges: DiffSpec[];
			}
		>({
			extraOptions: {
				command: "commit_amend",
				actionName: "Amend Commit",
			},
			query: ({ projectId, commitId, worktreeChanges }) => ({
				projectId,
				commitId,
				changes: worktreeChanges,
			}),
			transformResponse: (response: BackendCreateCommitOutcome) => {
				const normalizedResponse = normalizeCreateCommitOutcome(response);
				if (normalizedResponse.newCommit) {
					return normalizedResponse.newCommit;
				}

				const rejected = normalizedResponse.pathsToRejectedChanges
					.map(([reason, path]) => `${reason}: ${path}`)
					.join(", ");
				const details = rejected ? ` Rejected changes: ${rejected}` : "";
				throw new Error(`Failed to amend commit: no commit was created.${details}`);
			},
			invalidatesTags: [
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.BranchChanges),
				invalidatesList(ReduxTag.HeadSha),
			],
		}),
		absorbPlan: build.query<
			HunkAssignment.CommitAbsorption[],
			{ projectId: string; target: HunkAssignment.AbsorptionTarget }
		>({
			extraOptions: { command: "absorption_plan" },
			query: (args) => args,
		}),
		absorb: build.mutation<
			number,
			{ projectId: string; absorptionPlan: HunkAssignment.CommitAbsorption[] }
		>({
			extraOptions: {
				command: "absorb",
				actionName: "Absorb changes v2",
			},
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.HeadSha),
			],
		}),
		insertBlankCommit: build.mutation<
			string,
			{
				projectId: string;
				relativeTo: RelativeTo;
				side: "above" | "below";
			}
		>({
			extraOptions: {
				command: "commit_insert_blank",
				actionName: "Insert Blank Commit",
			},
			query: (args) => args,
			transformResponse: (response: BackendCommitInsertBlankResult) => response.newCommit,
			invalidatesTags: [invalidatesList(ReduxTag.HeadSha)],
		}),
		discardChanges: build.mutation<DiffSpec[], { projectId: string; worktreeChanges: DiffSpec[] }>({
			extraOptions: {
				command: "discard_worktree_changes",
				actionName: "Discard Changes",
			},
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.WorktreeChanges)],
		}),
		legacyMoveChangesBetweenCommits: build.mutation<
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
				command: "move_changes_between_commits",
				actionName: "Move Changes Between Commits",
			},
			query: (args) => args,
			invalidatesTags(result, _error, arg) {
				const commitChangesTags = [arg.sourceCommitId, arg.destinationCommitId]
					.map((id) => result?.replacedCommits.find(([oldId]) => oldId === id)?.[1])
					.filter(isDefined)
					.map((id) => invalidatesItem(ReduxTag.CommitChanges, id));
				return [
					invalidatesList(ReduxTag.HeadSha),
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesItem(ReduxTag.BranchChanges, arg.sourceStackId),
					invalidatesItem(ReduxTag.BranchChanges, arg.destinationStackId),
					...commitChangesTags,
				];
			},
		}),
		commitMoveChangesBetween: build.mutation<
			{
				replacedCommits: ReplacedCommit[];
			},
			{
				projectId: string;
				changes: DiffSpec[];
				sourceCommitId: string;
				destinationCommitId: string;
			}
		>({
			extraOptions: {
				command: "commit_move_changes_between",
				actionName: "Move Changes Between Commits",
			},
			query: (args) => args,
			transformResponse: (a: BackendMoveChangesResult) => ({
				replacedCommits: Object.entries(a.replacedCommits),
			}),
			invalidatesTags: [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.CommitChanges),
			],
		}),
		legacyUncommitChanges: build.mutation<
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
				command: "uncommit_changes",
				actionName: "Uncommit Changes",
			},
			query: (args) => args,
			invalidatesTags(_result, _error, args) {
				return [
					invalidatesList(ReduxTag.HeadSha),
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesItem(ReduxTag.BranchChanges, args.stackId),
				];
			},
		}),
		commitUncommitChanges: build.mutation<
			{
				replacedCommits: ReplacedCommit[];
			},
			{
				projectId: string;
				changes: DiffSpec[];
				commitId: string;
				assignTo?: string;
			}
		>({
			extraOptions: {
				command: "commit_uncommit_changes",
				actionName: "Uncommit Changes",
			},
			query: (args) => args,
			transformResponse: (a: BackendMoveChangesResult) => ({
				replacedCommits: Object.entries(a.replacedCommits),
			}),
			invalidatesTags() {
				return [
					invalidatesList(ReduxTag.HeadSha),
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesList(ReduxTag.BranchChanges),
				];
			},
		}),
		stashIntoBranch: build.mutation<
			DiffSpec[],
			{ projectId: string; branchName: string; worktreeChanges: DiffSpec[] }
		>({
			extraOptions: {
				command: "stash_into_branch",
				actionName: "Stash Changes",
			},
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.BranchListing),
			],
		}),
		unapply: build.mutation<void, { projectId: string; stackId: string }>({
			extraOptions: {
				command: "unapply_stack",
				actionName: "Unapply Stack",
			},
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.BranchListing),
			],
		}),
		// TODO: Why is this not part of the regular update call?
		updateBranchPrNumber: build.mutation<
			void,
			{
				projectId: string;
				stackId: string;
				branchName: string;
				prNumber?: number;
			}
		>({
			extraOptions: {
				command: "update_branch_pr_number",
				actionName: "Update Branch PR Number",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesItem(ReduxTag.StackDetails, args.stackId), // This probably is still needed
				invalidatesList(ReduxTag.BranchListing),
			],
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
				command: "update_branch_name",
				actionName: "Update Branch Name",
			},
			query: (args) => args,
			invalidatesTags: (_r, _e, args) => [
				invalidatesList(ReduxTag.Stacks), // Probably still needed
				invalidatesItem(ReduxTag.StackDetails, args.stackId), // This probably is still needed as well
				invalidatesList(ReduxTag.BranchListing),
			],
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
				command: "remove_branch",
				actionName: "Remove Branch",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesList(ReduxTag.Stacks), // Removing a branch can remove a stack
				// Removing a branch won't change the sha if the branch is empty
				invalidatesItem(ReduxTag.StackDetails, args.stackId),
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.BranchListing),
			],
		}),
		reorderStack: build.mutation<
			void,
			{ projectId: string; stackId: string; stackOrder: StackOrder }
		>({
			extraOptions: {
				command: "reorder_stack",
				actionName: "Reorder Stack",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesItem(ReduxTag.StackDetails, args.stackId), // This is probably still needed
			],
		}),
		moveCommit: build.mutation<
			MoveCommitIllegalAction | null,
			{ projectId: string; sourceStackId: string; commitId: string; targetStackId: string }
		>({
			extraOptions: {
				command: "move_commit",
				actionName: "Move Commit",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.WorktreeChanges), // Moving commits can cause conflicts
				invalidatesItem(ReduxTag.BranchChanges, args.sourceStackId),
				invalidatesItem(ReduxTag.BranchChanges, args.targetStackId),
			],
		}),
		moveBranch: build.mutation<
			MoveBranchResult,
			{
				projectId: string;
				sourceStackId: string;
				subjectBranchName: string;
				targetStackId: string;
				targetBranchName: string;
			}
		>({
			extraOptions: {
				command: "move_branch_legacy",
				actionName: "Move Branch",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.WorktreeChanges), // Moving commits can cause conflicts
				invalidatesItem(ReduxTag.BranchChanges, args.sourceStackId),
				invalidatesItem(ReduxTag.BranchChanges, args.targetStackId),
			],
		}),
		tearOffBranch: build.mutation<
			MoveBranchResult,
			{
				projectId: string;
				sourceStackId: string;
				subjectBranchName: string;
			}
		>({
			extraOptions: {
				command: "tear_off_branch_legacy",
				actionName: "Tear Off Branch",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => {
				return [
					invalidatesList(ReduxTag.HeadSha),
					invalidatesList(ReduxTag.WorktreeChanges), // Moving commits can cause conflicts
					invalidatesItem(ReduxTag.BranchChanges, args.sourceStackId), // Affects source stack, new stack is new
				];
			},
		}),
		integrateUpstreamCommits: build.mutation<
			void,
			{
				projectId: string;
				stackId: string;
				seriesName: string;
				integrationStrategy: SeriesIntegrationStrategy | undefined;
			}
		>({
			extraOptions: {
				command: "integrate_upstream_commits",
				actionName: "Integrate Upstream Commits",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesItem(ReduxTag.StackDetails, args.stackId),
				invalidatesItem(ReduxTag.BranchChanges, args.stackId),
			],
		}),
		getInitialIntegrationSteps: build.query<
			InteractiveIntegrationStep[],
			{ projectId: string; stackId: string | undefined; branchName: string }
		>({
			extraOptions: { command: "get_initial_integration_steps_for_branch" },
			query: (args) => args,
			providesTags: (_result, _error, { stackId, branchName }) =>
				providesItem(ReduxTag.IntegrationSteps, (stackId ?? "--no stack ID--") + branchName),
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
				command: "integrate_branch_with_steps",
				actionName: "Integrate Branch with Steps",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesItem(ReduxTag.StackDetails, args.stackId),
				invalidatesItem(ReduxTag.IntegrationSteps, args.stackId + args.branchName),
				invalidatesItem(ReduxTag.BranchDetails, args.branchName),
				invalidatesItem(ReduxTag.BranchChanges, args.stackId),
			],
		}),
		createVirtualBranchFromBranch: build.mutation<
			CreateBranchFromBranchOutcome,
			{ projectId: string; branch: string; remote?: string; prNumber?: number }
		>({
			extraOptions: {
				command: "create_virtual_branch_from_branch",
				actionName: "Create Virtual Branch From Branch",
			},
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.HeadSha), invalidatesList(ReduxTag.BranchListing)],
		}),
		deleteLocalBranch: build.mutation<
			void,
			{ projectId: string; refname: string; givenName: string }
		>({
			extraOptions: {
				command: "delete_local_branch",
				actionName: "Delete Local Branch",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, { givenName: branchName }) => [
				invalidatesItem(ReduxTag.BranchDetails, branchName),
				providesList(ReduxTag.BranchListing),
			],
		}),
		squashCommits: build.mutation<
			void,
			{ projectId: string; stackId: string; sourceCommitIds: string[]; targetCommitId: string }
		>({
			extraOptions: {
				command: "squash_commits",
				actionName: "Squash Commits",
			},
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.WorktreeChanges), // Could cause conflicts
			],
		}),
		newBranchName: build.query<
			string,
			{
				projectId: string;
			}
		>({
			extraOptions: { command: "canned_branch_name" },
			query: (args) => args,
		}),
		normalizeBranchName: build.query<
			string,
			{
				name: string;
			}
		>({
			extraOptions: { command: "normalize_branch_name" },
			query: (args) => args,
		}),
		targetCommits: build.query<
			EntityState<Commit, string>,
			{
				projectId: string;
				lastCommitId: string | undefined;
				pageSize: number;
			}
		>({
			extraOptions: { command: "target_commits" },
			query: (args) => args,
			transformResponse: (commits: Commit[]) =>
				commitAdapter.addMany(commitAdapter.getInitialState(), commits),
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
				command: "split_branch",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesItem(ReduxTag.BranchChanges, args.sourceStackId),
			],
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
				command: "split_branch_into_dependent_branch",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesItem(ReduxTag.BranchChanges, args.sourceStackId),
			],
		}),
		createReference: build.mutation<
			void,
			{ projectId: string; stackId: string; request: CreateRefRequest }
		>({
			extraOptions: {
				command: "create_reference",
				actionName: "Create Reference",
			},
			query: (args) => {
				// TODO: Remove the stack ID from the request args.
				// The backend doesn't need it, but the frontend does to invalidate the right tags.
				// We should move away from using the stack ID as the cache key, an move towards some form of branch name instead.

				return { projectId: args.projectId, request: args.request };
			},
			invalidatesTags: (_result, _error, args) => [
				invalidatesItem(ReduxTag.StackDetails, args.stackId), // This is probably still needed. Adding a ref won't change the workspace commit, right?
			],
		}),
		templates: build.query<string[], { projectId: string; forge: string }>({
			extraOptions: { command: "pr_templates" },
			query: (args) => args,
		}),
		template: build.query<string, { projectId: string; forge: string; relativePath: string }>({
			extraOptions: { command: "pr_template" },
			query: (args) => args,
		}),
	};
}
