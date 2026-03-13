import type {
	ApplyOutcome,
	AssignmentRejection,
	BranchDetails,
	BranchListing,
	BranchListingFilter,
	HunkAssignmentRequest,
	CommitDetails,
	DiffSpec,
	InsertSide,
	ProjectForFrontend,
	RelativeTo,
	RefInfo,
	TreeChange,
	TreeChanges,
	UICommitCreateResult,
	UICommitInsertBlankResult,
	UICommitMoveResult,
	UICommitRewordResult,
	UIMoveChangesResult,
	UnifiedPatch,
	WorktreeChanges,
} from "@gitbutler/but-sdk";

export interface AssignHunkParams {
	projectId: string;
	assignments: Array<HunkAssignmentRequest>;
}

export interface ApplyParams {
	projectId: string;
	existingBranch: string;
}

export interface BranchDetailsParams {
	projectId: string;
	branchName: string;
	remote: string | null;
}

export interface BranchDiffParams {
	projectId: string;
	branch: string;
}

export interface CommitAmendParams {
	projectId: string;
	commitId: string;
	changes: Array<DiffSpec>;
}

export interface CommitDetailsWithLineStatsParams {
	projectId: string;
	commitId: string;
}

export interface CommitCreateParams {
	projectId: string;
	relativeTo: RelativeTo;
	side: InsertSide;
	changes: Array<DiffSpec>;
	message: string;
}

export interface CommitMoveChangesBetweenParams {
	projectId: string;
	sourceCommitId: string;
	destinationCommitId: string;
	changes: Array<DiffSpec>;
}

export interface CommitMoveParams {
	projectId: string;
	subjectCommitId: string;
	anchorCommitId: string;
	side: InsertSide;
}

export interface CommitMoveToBranchParams {
	projectId: string;
	subjectCommitId: string;
	anchorRef: string;
}

export interface CommitInsertBlankParams {
	projectId: string;
	relativeTo: RelativeTo;
	side: InsertSide;
}

export interface CommitRewordParams {
	projectId: string;
	commitId: string;
	message: string;
}

export interface CommitUncommitChangesParams {
	projectId: string;
	commitId: string;
	changes: Array<DiffSpec>;
	assignTo: string | null;
}

export interface TreeChangeDiffParams {
	projectId: string;
	change: TreeChange;
}

export interface UnapplyStackParams {
	projectId: string;
	stackId: string;
}

export interface LiteElectronApi {
	apply: (params: ApplyParams) => Promise<ApplyOutcome>;
	assignHunk: (params: AssignHunkParams) => Promise<Array<AssignmentRejection>>;
	branchDetails: (params: BranchDetailsParams) => Promise<BranchDetails>;
	branchDiff: (params: BranchDiffParams) => Promise<TreeChanges>;
	changesInWorktree: (projectId: string) => Promise<WorktreeChanges>;
	commitAmend: (params: CommitAmendParams) => Promise<UICommitCreateResult>;
	commitCreate: (params: CommitCreateParams) => Promise<UICommitCreateResult>;
	commitDetailsWithLineStats: (params: CommitDetailsWithLineStatsParams) => Promise<CommitDetails>;
	commitInsertBlank: (params: CommitInsertBlankParams) => Promise<UICommitInsertBlankResult>;
	commitMove: (params: CommitMoveParams) => Promise<UICommitMoveResult>;
	commitMoveToBranch: (params: CommitMoveToBranchParams) => Promise<UICommitMoveResult>;
	commitReword: (params: CommitRewordParams) => Promise<UICommitRewordResult>;
	commitMoveChangesBetween: (
		params: CommitMoveChangesBetweenParams,
	) => Promise<UIMoveChangesResult>;
	commitUncommitChanges: (params: CommitUncommitChangesParams) => Promise<UIMoveChangesResult>;
	getVersion: () => Promise<string>;
	headInfo: (projectId: string) => Promise<RefInfo>;
	listBranches: (
		projectId: string,
		filter: BranchListingFilter | null,
	) => Promise<Array<BranchListing>>;
	listProjects: () => Promise<Array<ProjectForFrontend>>;
	ping: (input: string) => Promise<string>;
	treeChangeDiffs: (params: TreeChangeDiffParams) => Promise<UnifiedPatch | null>;
	unapplyStack: (params: UnapplyStackParams) => Promise<void>;
}

export const liteIpcChannels = {
	apply: "workspace:apply",
	assignHunk: "workspace:assign-hunk",
	branchDetails: "workspace:branch-details",
	branchDiff: "workspace:branch-diff",
	changesInWorktree: "workspace:changes-in-worktree",
	commitAmend: "workspace:commit-amend",
	commitCreate: "workspace:commit-create",
	commitDetailsWithLineStats: "workspace:commit-details-with-line-stats",
	commitInsertBlank: "workspace:commit-insert-blank",
	commitMove: "workspace:commit-move",
	commitMoveToBranch: "workspace:commit-move-to-branch",
	commitReword: "workspace:commit-reword",
	commitMoveChangesBetween: "workspace:commit-move-changes-between",
	commitUncommitChanges: "workspace:commit-uncommit-changes",
	getVersion: "lite:get-version",
	headInfo: "workspace:head-info",
	listBranches: "workspace:list-branches",
	listProjects: "projects:list",
	ping: "lite:ping",
	treeChangeDiffs: "workspace:tree-change-diffs",
	unapplyStack: "workspace:unapply-stack",
} as const;
