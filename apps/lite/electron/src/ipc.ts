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

export interface LiteElectronApi {
	assignHunk(
		projectId: string,
		assignments: Array<HunkAssignmentRequest>,
	): Promise<Array<AssignmentRejection>>;
	changesInWorktree(projectId: string): Promise<WorktreeChanges>;
	commitAmend(
		projectId: string,
		commitId: string,
		changes: Array<DiffSpec>,
	): Promise<UICommitCreateResult>;
	commitDetailsWithLineStats(projectId: string, commitId: string): Promise<CommitDetails>;
	commitMoveChangesBetween(
		projectId: string,
		sourceCommitId: string,
		destinationCommitId: string,
		changes: Array<DiffSpec>,
	): Promise<UIMoveChangesResult>;
	commitUncommitChanges(
		projectId: string,
		commitId: string,
		changes: Array<DiffSpec>,
		assignTo: string | null,
	): Promise<UIMoveChangesResult>;
	getVersion(): Promise<string>;
	headInfo(projectId: string): Promise<RefInfo>;
	listProjects(): Promise<Array<ProjectForFrontend>>;
	ping(input: string): Promise<string>;
	treeChangeDiffs(projectId: string, change: TreeChange): Promise<UnifiedPatch | null>;
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
