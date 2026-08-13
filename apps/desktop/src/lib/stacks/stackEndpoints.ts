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
	invalidatesType,
	providesItem,
	providesItems,
	providesList,
	ReduxTag,
} from "$lib/state/tags";
import { createEntityAdapter, type EntityState } from "@reduxjs/toolkit";
import type { Stack, GerritPushFlag } from "$lib/stacks/stack";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";
import type {
	AbsorptionTarget,
	AiResolutionResult,
	BranchLandResult,
	CommitAbsorption,
	BranchDetails,
	UpstreamCommit,
	Commit,
	InitialBranchIntegration,
	BranchIntegrationStrategy,
	InteractiveIntegration,
	IntegrateBranchResult,
	TreeChange,
	TreeStats,
	TreeChanges,
	CommitDetails,
	DiffSpec,
	MoveChangesResult,
	CommitCherryPickResult,
	CommitCreateResult,
	CommitRewordResult,
	CommitSquashResult,
	CommitInsertBlankResult,
	ApplyOutcome,
	MoveBranchResult,
	RejectionReason,
	UncommitResult,
	InsertSide,
	RelativeTo,
	RefInfo,
	StackEntryNoOpt,
	BottomUpdate,
	WorkspaceIntegrateUpstreamOutcome,
	BranchCreatePlacement,
	BranchCreateResult,
	BranchRemoveResult,
	UncommitChangesFromCommitsResult,
	UncommitChangesSource,
	BranchRenameResult,
	PushResult,
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

export type BranchPushResult = PushResult;

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
		workspaceIntegrateUpstream: build.mutation<
			WorkspaceIntegrateUpstreamOutcome,
			{ projectId: string; updates: BottomUpdate[]; dryRun: boolean }
		>({
			extraOptions: {
				command: "workspace_integrate_upstream",
				actionName: "Update Workspace",
			},
			query: (args) => args,
			invalidatesTags: (_result, _error, args) => {
				if (args.dryRun) return [];

				return [
					invalidatesList(ReduxTag.HeadSha),
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.StackDetails),
					invalidatesList(ReduxTag.BranchChanges),
					invalidatesList(ReduxTag.BranchListing),
					invalidatesType(ReduxTag.BaseBranchData),
				];
			},
		}),
		createStack: build.mutation<StackEntryNoOpt, { projectId: string; branch: BranchParams }>({
			extraOptions: {
				command: "create_virtual_branch",
				actionName: "Create Stack",
			},
			query: (args) => args,
			invalidatesTags: (result, _error) => [
				invalidatesItem(ReduxTag.StackDetails, result?.id || "undefined"),
				invalidatesList(ReduxTag.Stacks),
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
				invalidatesList(ReduxTag.StackDetails),
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
					changesSource: { type: "head" },
					message: args.message,
					dryRun: args.dryRun,
				};
			},
			transformResponse: normalizeCreateCommitOutcome,
			invalidatesTags: [
				invalidatesList(ReduxTag.WorktreeChanges),
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
		resolveCommitConflictsAi: build.mutation<
			AiResolutionResult,
			{ projectId: string; stackId?: string; commitId: string }
		>({
			extraOptions: {
				command: "resolve_commit_conflicts_ai",
				actionName: "Resolve Conflicts with AI",
			},
			query: ({ projectId, commitId }) => ({
				projectId,
				commitId,
				dryRun: false,
			}),
			invalidatesTags: (_result, _error, { stackId }) => [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.BranchChanges),
				invalidatesList(ReduxTag.WorktreeChanges),
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
		branchCreate: build.mutation<
			BranchCreateResult,
			{ projectId: string; newRef: string | null; placement: BranchCreatePlacement }
		>({
			extraOptions: {
				command: "branch_create",
				actionName: "Create Branch",
			},
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
				invalidatesList(ReduxTag.BranchListing),
			],
		}),
		branchRemove: build.mutation<BranchRemoveResult, { projectId: string; refName: number[] }>({
			extraOptions: {
				command: "branch_remove",
				actionName: "Remove Branch",
			},
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.Stacks), // Removing a branch can remove a stack
				invalidatesList(ReduxTag.StackDetails),
				invalidatesList(ReduxTag.BranchListing),
			],
		}),
		branchRename: build.mutation<
			BranchRenameResult,
			{
				projectId: string;
				refName: number[];
				newName: string;
				// Carried for the optimistic UI side effect only; not sent to the backend.
				// (`newName` above IS sent to the backend as part of the rename payload.)
				laneId?: string;
				branchName?: string;
			}
		>({
			extraOptions: {
				command: "branch_rename",
				actionName: "Rename Branch",
			},
			query: ({ projectId, refName, newName }) => ({ projectId, refName, newName }),
			invalidatesTags: [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
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
				changesSource: { type: "head" },
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
		commitUncommitChangesFromCommits: build.mutation<
			UncommitChangesFromCommitsResult,
			{
				projectId: string;
				sources: UncommitChangesSource[];
				assignTo?: string;
				dryRun: boolean;
			}
		>({
			extraOptions: {
				command: "commit_uncommit_changes_from_commits",
				actionName: "Uncommit Changes",
			},
			query: ({ projectId, sources, assignTo, dryRun }) => ({
				projectId,
				sources,
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
				invalidatesList(ReduxTag.Stacks),
			],
		}),
		/**
		 * Copies commits from anywhere in the repository into the workspace.
		 *
		 * Placement follows the same `relativeTo`/`side` rules as `commitMove`,
		 * so targeting a branch reference with `side: "below"` copies the commits
		 * onto the tip of that stack.
		 */
		commitCherryPick: build.mutation<
			CommitCherryPickResult,
			{
				projectId: string;
				sourceCommitIds: string[];
				relativeTo: RelativeTo;
				side: InsertSide;
				dryRun: boolean;
			}
		>({
			extraOptions: {
				command: "commit_cherry_pick",
				actionName: "Cherry-pick Commit",
			},
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.WorktreeChanges), // Cherry-picking can cause conflicts
				invalidatesList(ReduxTag.BranchChanges),
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
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
				// Reordering empty branches in single-branch mode is metadata-only and doesn't move
				// HEAD, so the stack/branch list must be invalidated explicitly to reflect the new order.
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
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
		landBranch: build.mutation<
			BranchLandResult,
			{ projectId: string; branch: string; noFf: boolean; wholeStack: boolean }
		>({
			extraOptions: {
				command: "branch_land",
				actionName: "Land Branch",
			},
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
				invalidatesList(ReduxTag.BranchChanges),
				invalidatesList(ReduxTag.BranchListing),
				invalidatesType(ReduxTag.BaseBranchData),
			],
		}),
		getInitialBranchIntegration: build.query<
			InitialBranchIntegration,
			{ projectId: string; branchRef: string; strategy: BranchIntegrationStrategy | null }
		>({
			extraOptions: { command: "get_initial_branch_integration" },
			query: ({ projectId, branchRef, strategy }) => ({
				projectId,
				branch: branchRef,
				strategy,
			}),
			providesTags: [providesList(ReduxTag.IntegrationSteps)],
		}),
		applyBranchIntegration: build.mutation<
			IntegrateBranchResult,
			{
				projectId: string;
				branchRef: string;
				integration: InteractiveIntegration;
				dryRun: boolean;
			}
		>({
			extraOptions: {
				command: "apply_branch_integration",
				actionName: "Apply Branch Integration",
			},
			query: ({ projectId, branchRef, integration, dryRun }) => ({
				projectId,
				branch: branchRef,
				integration,
				dryRun,
			}),
			invalidatesTags: (_result, _error, args) =>
				args.dryRun
					? []
					: [
							invalidatesList(ReduxTag.HeadSha),
							invalidatesList(ReduxTag.WorktreeChanges),
							invalidatesList(ReduxTag.Stacks),
							invalidatesList(ReduxTag.StackDetails),
							invalidatesList(ReduxTag.BranchListing),
						],
		}),
		branchApply: build.mutation<ApplyOutcome, { projectId: string; existingBranch: string }>({
			extraOptions: {
				command: "apply",
				actionName: "Apply Branch",
			},
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.HeadMetadata),
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
				invalidatesList(ReduxTag.BranchListing),
			],
		}),
		reviewApply: build.mutation<ApplyOutcome, { projectId: string; reviewId: number }>({
			extraOptions: {
				command: "review_apply",
				actionName: "Apply Review",
			},
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.HeadMetadata),
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.Stacks),
				invalidatesList(ReduxTag.StackDetails),
				invalidatesList(ReduxTag.BranchListing),
				invalidatesList(ReduxTag.PullRequests),
			],
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
