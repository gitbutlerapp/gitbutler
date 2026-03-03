import type {
	AssignmentRejection,
	HunkAssignmentRequest,
	CommitDetails,
	DiffSpec,
	ProjectForFrontend,
	RefInfo,
	TreeChange,
	UICommitCreateResult,
	UIMoveChangesResult,
	UnifiedPatch,
	WorktreeChanges,
} from "@gitbutler/but-sdk";

export interface AssignHunkParams {
	projectId: string;
	assignments: Array<HunkAssignmentRequest>;
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

export interface CommitMoveChangesBetweenParams {
	projectId: string;
	sourceCommitId: string;
	destinationCommitId: string;
	changes: Array<DiffSpec>;
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

export interface LiteElectronApi {
	assignHunk(params: AssignHunkParams): Promise<Array<AssignmentRejection>>;
	changesInWorktree(projectId: string): Promise<WorktreeChanges>;
	commitAmend(params: CommitAmendParams): Promise<UICommitCreateResult>;
	commitDetailsWithLineStats(params: CommitDetailsWithLineStatsParams): Promise<CommitDetails>;
	commitMoveChangesBetween(params: CommitMoveChangesBetweenParams): Promise<UIMoveChangesResult>;
	commitUncommitChanges(params: CommitUncommitChangesParams): Promise<UIMoveChangesResult>;
	getVersion(): Promise<string>;
	headInfo(projectId: string): Promise<RefInfo>;
	listProjects(): Promise<Array<ProjectForFrontend>>;
	ping(input: string): Promise<string>;
	treeChangeDiffs(params: TreeChangeDiffParams): Promise<UnifiedPatch | null>;
}

export const liteIpcChannels = {
	assignHunk: "workspace:assign-hunk",
	changesInWorktree: "workspace:changes-in-worktree",
	commitAmend: "workspace:commit-amend",
	commitDetailsWithLineStats: "workspace:commit-details-with-line-stats",
	commitMoveChangesBetween: "workspace:commit-move-changes-between",
	commitUncommitChanges: "workspace:commit-uncommit-changes",
	getVersion: "lite:get-version",
	headInfo: "workspace:head-info",
	listProjects: "projects:list",
	ping: "lite:ping",
	treeChangeDiffs: "workspace:tree-change-diffs",
} as const;
