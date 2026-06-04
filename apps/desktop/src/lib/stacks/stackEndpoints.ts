import { ConflictEntries, type ConflictEntriesObj } from "$lib/files/conflicts";
import { normalizeReferenceSubject } from "$lib/stacks/commitMovePlacement";
import {
	transformWorkspaceDetails,
	workspaceStackDetailTags,
	type WorkspaceDetails,
} from "$lib/stacks/headInfoAdapters";
import { createSelectByIds, createSelectNth } from "$lib/state/customSelectors";
import {
	invalidatesItem,
	invalidatesList,
	providesItem,
	providesItems,
	providesList,
	ReduxTag,
} from "$lib/state/tags";
import { createEntityAdapter, type EntityState } from "@reduxjs/toolkit";
import type {
	Stack,
	CreateRefRequest,
	InteractiveIntegrationStep,
	CreateBranchFromBranchOutcome,
	GerritPushFlag,
} from "$lib/stacks/stack";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";
import type {
	AbsorptionTarget,
	CommitAbsorption,
	BranchDetails,
	UpstreamCommit,
	Commit,
	TreeChange,
	TreeStats,
	TreeChanges,
	CommitDetails,
	DiffSpec,
	MoveChangesResult,
	CommitCreateResult,
	CommitRewordResult,
	CommitSquashResult,
	CommitInsertBlankResult,
	MoveBranchResult,
	RejectionReason,
	UncommitResult,
	InsertSide,
	RelativeTo,
	RefInfo,
	StackEntry,
} from "@gitbutler/but-sdk";

export type BranchParams = {
	name?: string;
	order?: number;
};

export type CreateCommitRequest = {
	message: string;
	/** Undefined means that the backend will infer the parent to be the current head of stackBranchName */
	parentId: string | undefined;
	/** When true, insert below `parentId` instead of above it. */
	insertBelow?: boolean;
	stackBranchName: string;
	worktreeChanges: DiffSpec[];
	dryRun: boolean;
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
	"workspaceMergeConflictOfUnrelatedFile",
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

type BackendRejectedChange = {
	reason: RejectionReason;
	path: string;
};

export function readableRejectionReason(reason: RejectionReason): string {
	switch (reason) {
		case "cherryPickMergeConflict":
			return "Cherry-pick merge conflict";
		case "noEffectiveChanges":
			return "No effective changes";
		case "workspaceMergeConflict":
			return "Workspace merge conflict";
		case "workspaceMergeConflictOfUnrelatedFile":
			return "Workspace merge conflict of unrelated file";
		case "worktreeFileMissingForObjectConversion":
			return "Worktree file missing for object conversion";
		case "fileToLargeOrBinary":
			return "File too large or binary";
		case "pathNotFoundInBaseTree":
			return "Path not found in base tree";
		case "unsupportedDirectoryEntry":
			return "Unsupported directory entry";
		case "unsupportedTreeEntry":
			return "Unsupported tree entry";
		case "missingDiffSpecAssociation":
			return "Missing diff spec association";
	}
}

export type CreateCommitOutcome = {
	newCommit: string | null;
	rejectedChanges: BackendRejectedChange[];
	commitMapping: ReplacedCommit[];
};

export function normalizeCreateCommitOutcome(response: CommitCreateResult): CreateCommitOutcome {
	return {
		newCommit: response.newCommit ?? null,
		rejectedChanges: response.rejectedChanges,
		commitMapping: Object.entries(response.workspace.replacedCommits),
	};
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
			side: args.insertBelow ? "below" : "above",
		};
	}

	return {
		relativeTo: {
			type: "reference",
			subject: normalizeReferenceSubject(args.stackBranchName),
		},
		side: "below",
	};
}
// Entity adapters and selectors

export const stackAdapter = createEntityAdapter<Stack, string>({
	selectId: (stack) => stack.id ?? stack.segments.at(0)?.refName?.displayName ?? stack.base ?? "",
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

export function buildStackEndpoints(build: BackendEndpointBuilder) {
	return {
		workspaceDetails: build.query<WorkspaceDetails, { projectId: string }>({
			extraOptions: { command: "head_info" },
			query: (args) => args,
			providesTags: (result) => {
				const stackIds = result ? workspaceStackDetailTags(result) : [];
				return [providesList(ReduxTag.Stacks), ...providesItems(ReduxTag.StackDetails, stackIds)];
			},
			transformResponse(response: RefInfo) {
				return transformWorkspaceDetails(response);
			},
		}),
		createStack: build.mutation<StackEntry, { projectId: string; branch: BranchParams }>({
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
		/**
		 * Note: This is specifically for looking up branches outside of
		 * a stacking context. Stacked workspace branches should be read from
		 * the `head_info`-backed workspace details query.
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
		pushWorkspaceBranchAndAncestors: build.mutation<
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
				command: "workspace_branch_and_ancestors_push",
				actionName: "Push",
			},
			query: ({ stackId: _stackId, branch, ...args }) => ({
				branch: branch.startsWith("refs/") ? branch : `refs/heads/${branch}`,
				...args,
			}),
			invalidatesTags: (_result, _error, args) => [
				invalidatesItem(ReduxTag.StackDetails, args.stackId),
				invalidatesList(ReduxTag.BranchListing),
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
					dryRun: args.dryRun,
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
				stats: TreeStats | null;
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
			{ projectId: string; branch: string }
		>({
			extraOptions: { command: "branch_diff" },
			query: (args) => args,
			providesTags: (_result, _error, { branch }) => providesItem(ReduxTag.BranchChanges, branch),
			transformResponse(rsp: TreeChanges) {
				return {
					changes: changesAdapter.addMany(changesAdapter.getInitialState(), rsp.changes),
					stats: rsp.stats,
				};
			},
		}),
		updateCommitMessage: build.mutation<
			string,
			{ projectId: string; stackId?: string; commitId: string; message: string; dryRun: boolean }
		>({
			extraOptions: {
				command: "commit_reword",
				actionName: "Update Commit Message",
			},
			query: ({ projectId, commitId, message, dryRun }) => ({
				projectId,
				commitId,
				message,
				dryRun,
			}),
			transformResponse: (response: CommitRewordResult) => response.newCommit,
			invalidatesTags: (_result, _error, { stackId }) => [
				invalidatesList(ReduxTag.HeadSha),
				...(stackId ? [invalidatesItem(ReduxTag.StackDetails, stackId)] : []),
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
		uncommit: build.mutation<
			UncommitResult,
			{ projectId: string; stackId?: string; commitIds: string[] }
		>({
			extraOptions: {
				command: "commit_uncommit",
				actionName: "Uncommit",
			},
			query: ({ projectId, stackId, commitIds }) => ({
				projectId,
				subjectCommitIds: commitIds,
				assignTo: stackId ?? null,
				dryRun: false,
			}),
			invalidatesTags: [
				invalidatesList(ReduxTag.BranchChanges),
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.HeadSha),
			],
		}),
		commitAmend: build.mutation<
			CreateCommitOutcome,
			{
				projectId: string;
				commitId: string;
				worktreeChanges: DiffSpec[];
				dryRun: boolean;
			}
		>({
			extraOptions: {
				command: "commit_amend",
				actionName: "Amend Commit",
			},
			query: ({ projectId, commitId, worktreeChanges, dryRun }) => ({
				projectId,
				commitId,
				changes: worktreeChanges,
				dryRun,
			}),
			transformResponse: normalizeCreateCommitOutcome,
			invalidatesTags: [
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.BranchChanges),
				invalidatesList(ReduxTag.HeadSha),
			],
		}),
		absorbPlan: build.query<CommitAbsorption[], { projectId: string; target: AbsorptionTarget }>({
			extraOptions: { command: "absorption_plan" },
			query: (args) => args,
		}),
		absorb: build.mutation<number, { projectId: string; absorptionPlan: CommitAbsorption[] }>({
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
				dryRun: boolean;
			}
		>({
			extraOptions: {
				command: "commit_insert_blank",
				actionName: "Insert Blank Commit",
			},
			query: ({ projectId, relativeTo, side, dryRun }) => ({
				projectId,
				relativeTo,
				side,
				dryRun,
			}),
			transformResponse: (response: CommitInsertBlankResult) => response.newCommit,
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
		commitMoveChangesBetween: build.mutation<
			MoveChangesResult,
			{
				projectId: string;
				changes: DiffSpec[];
				sourceCommitId: string;
				destinationCommitId: string;
				dryRun: boolean;
			}
		>({
			extraOptions: {
				command: "commit_move_changes_between",
				actionName: "Move Changes Between Commits",
			},
			query: ({ projectId, changes, sourceCommitId, destinationCommitId, dryRun }) => ({
				projectId,
				changes,
				sourceCommitId,
				destinationCommitId,
				dryRun,
			}),
			invalidatesTags: [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.CommitChanges),
			],
		}),
		commitUncommitChanges: build.mutation<
			MoveChangesResult,
			{
				projectId: string;
				changes: DiffSpec[];
				commitId: string;
				assignTo?: string;
				dryRun: boolean;
			}
		>({
			extraOptions: {
				command: "commit_uncommit_changes",
				actionName: "Uncommit Changes",
			},
			query: ({ projectId, changes, commitId, assignTo, dryRun }) => ({
				projectId,
				changes,
				commitId,
				assignTo,
				dryRun,
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
		/**
		 * Generic commit move wrapper around `commit_move` for both reorder and
		 * cross-stack drag/drop flows.
		 *
		 * Callers must provide the exact placement using `relativeTo` and `side`.
		 * Targeting a branch reference with `side: "below"` inserts the commit at
		 * the top of that destination stack.
		 */
		commitMove: build.mutation<
			void,
			{
				projectId: string;
				subjectCommitIds: Array<string>;
				relativeTo: RelativeTo;
				side: InsertSide;
				dryRun: boolean;
			}
		>({
			extraOptions: {
				command: "commit_move",
				actionName: "Move Commit",
			},
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.WorktreeChanges), // Moving commits can cause conflicts
				invalidatesList(ReduxTag.BranchChanges),
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
			],
		}),
		moveBranch: build.mutation<
			MoveBranchResult,
			{
				projectId: string;
				subjectBranch: string;
				targetBranch: string;
			}
		>({
			extraOptions: {
				command: "move_branch",
				actionName: "Move Branch",
			},
			query: ({ projectId, subjectBranch, targetBranch }) => ({
				projectId,
				subjectBranch,
				targetBranch,
				dryRun: false,
			}),
			invalidatesTags: [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.WorktreeChanges), // Moving commits can cause conflicts
				invalidatesList(ReduxTag.BranchChanges),
			],
		}),
		tearOffBranch: build.mutation<
			MoveBranchResult,
			{
				projectId: string;
				sourceStackId?: string;
				subjectBranchName: string;
			}
		>({
			extraOptions: {
				command: "tear_off_branch",
				actionName: "Tear Off Branch",
			},
			query: ({ projectId, subjectBranchName }) => ({
				projectId,
				subjectBranch: normalizeReferenceSubject(subjectBranchName),
				dryRun: false,
			}),
			invalidatesTags: (_result, _error, args) => [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.WorktreeChanges), // Moving commits can cause conflicts
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.BranchChanges),
				...(args.sourceStackId ? [invalidatesItem(ReduxTag.StackDetails, args.sourceStackId)] : []),
			],
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
				invalidatesItem(ReduxTag.BranchChanges, args.seriesName),
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
				invalidatesItem(ReduxTag.BranchChanges, args.branchName),
			],
		}),
		createVirtualBranchFromBranch: build.mutation<
			CreateBranchFromBranchOutcome,
			{ projectId: string; branch: string; prNumber?: number }
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
			CommitSquashResult,
			{ projectId: string; sourceCommitIds: string[]; targetCommitId: string }
		>({
			extraOptions: {
				command: "commit_squash",
				actionName: "Squash Commits",
			},
			query: ({ projectId, sourceCommitIds, targetCommitId }) => ({
				projectId,
				subjectCommitIds: sourceCommitIds,
				targetCommitId,
				howToCombineMessages: "KeepBoth",
				dryRun: false,
			}),
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
		createReference: build.mutation<
			void,
			{ projectId: string; stackId?: string; request: CreateRefRequest }
		>({
			extraOptions: {
				command: "create_reference",
				actionName: "Create Reference",
			},
			query: (args) => ({ projectId: args.projectId, request: args.request }),
			invalidatesTags: (_result, _error, args) =>
				args.stackId ? [invalidatesItem(ReduxTag.StackDetails, args.stackId)] : [],
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
