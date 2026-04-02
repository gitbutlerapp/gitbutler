import type {
	AbsorptionTarget,
	ApplyOutcome,
	BranchDetails,
	BranchListing,
	BranchListingFilter,
	CommitAbsorption,
	HunkAssignmentRequest,
	CommitDetails,
	DiffSpec,
	InsertSide,
	PushResult,
	ProjectForFrontend,
	RelativeTo,
	RefInfo,
	TreeChange,
	TreeChanges,
	UICommitCreateResult,
	UICommitDiscardResult,
	UICommitInsertBlankResult,
	UICommitMoveResult,
	UICommitRewordResult,
	UIMoveBranchResult,
	UIMoveChangesResult,
	UnifiedPatch,
	WatcherEvent,
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

export interface MoveBranchParams {
	projectId: string;
	subjectBranch: string;
	targetBranch: string;
}

export interface UpdateBranchNameParams {
	projectId: string;
	stackId: string;
	branchName: string;
	newName: string;
}

export interface RemoveBranchParams {
	projectId: string;
	stackId: string;
	branchName: string;
}

export interface TearOffBranchParams {
	projectId: string;
	subjectBranch: string;
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

export interface CommitDiscardParams {
	projectId: string;
	subjectCommitId: string;
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
	relativeTo: RelativeTo;
	side: InsertSide;
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

export interface AbsorptionPlanParams {
	projectId: string;
	target: AbsorptionTarget;
}

export interface AbsorbParams {
	projectId: string;
	absorptionPlan: Array<CommitAbsorption>;
}

export interface TreeChangeDiffParams {
	projectId: string;
	change: TreeChange;
}

export interface UnapplyStackParams {
	projectId: string;
	stackId: string;
}

export interface PushStackLegacyParams {
	projectId: string;
	stackId: string;
	branch: string;
}

export interface WatcherSubscribeParams {
	projectId: string;
}

export interface WatcherUnsubscribeParams {
	subscriptionId: string;
}

export interface WatcherSubscribeResult {
	subscriptionId: string;
	eventChannel: string;
}

export interface LiteElectronApi {
	absorptionPlan: (params: AbsorptionPlanParams) => Promise<Array<CommitAbsorption>>;
	absorb: (params: AbsorbParams) => Promise<number>;
	apply: (params: ApplyParams) => Promise<ApplyOutcome>;
	assignHunk: (params: AssignHunkParams) => Promise<void>;
	branchDetails: (params: BranchDetailsParams) => Promise<BranchDetails>;
	branchDiff: (params: BranchDiffParams) => Promise<TreeChanges>;
	changesInWorktree: (projectId: string) => Promise<WorktreeChanges>;
	commitAmend: (params: CommitAmendParams) => Promise<UICommitCreateResult>;
	commitCreate: (params: CommitCreateParams) => Promise<UICommitCreateResult>;
	commitDiscard: (params: CommitDiscardParams) => Promise<UICommitDiscardResult>;
	commitDetailsWithLineStats: (params: CommitDetailsWithLineStatsParams) => Promise<CommitDetails>;
	commitInsertBlank: (params: CommitInsertBlankParams) => Promise<UICommitInsertBlankResult>;
	commitMove: (params: CommitMoveParams) => Promise<UICommitMoveResult>;
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
	moveBranch: (params: MoveBranchParams) => Promise<UIMoveBranchResult>;
	removeBranch: (params: RemoveBranchParams) => Promise<void>;
	updateBranchName: (params: UpdateBranchNameParams) => Promise<void>;
	tearOffBranch: (params: TearOffBranchParams) => Promise<UIMoveBranchResult>;
	ping: (input: string) => Promise<string>;
	pushStackLegacy: (params: PushStackLegacyParams) => Promise<PushResult>;
	treeChangeDiffs: (params: TreeChangeDiffParams) => Promise<UnifiedPatch | null>;
	unapplyStack: (params: UnapplyStackParams) => Promise<void>;
	watcherSubscribe: (projectId: string, callback: (event: WatcherEvent) => void) => Promise<string>;
	watcherUnsubscribe: (subscriptionId: string) => Promise<boolean>;
	watcherStopAll: () => Promise<number>;
}

export const liteIpcChannels = {
	absorptionPlan: "workspace:absorption-plan",
	absorb: "workspace:absorb",
	apply: "workspace:apply",
	assignHunk: "workspace:assign-hunk",
	branchDetails: "workspace:branch-details",
	branchDiff: "workspace:branch-diff",
	changesInWorktree: "workspace:changes-in-worktree",
	commitAmend: "workspace:commit-amend",
	commitCreate: "workspace:commit-create",
	commitDiscard: "workspace:commit-discard",
	commitDetailsWithLineStats: "workspace:commit-details-with-line-stats",
	commitInsertBlank: "workspace:commit-insert-blank",
	commitMove: "workspace:commit-move",
	commitReword: "workspace:commit-reword",
	commitMoveChangesBetween: "workspace:commit-move-changes-between",
	commitUncommitChanges: "workspace:commit-uncommit-changes",
	getVersion: "lite:get-version",
	headInfo: "workspace:head-info",
	listBranches: "workspace:list-branches",
	listProjects: "projects:list",
	moveBranch: "workspace:move-branch",
	removeBranch: "workspace:remove-branch",
	updateBranchName: "workspace:update-branch-name",
	tearOffBranch: "workspace:tear-off-branch",
	ping: "lite:ping",
	pushStackLegacy: "workspace:push-stack-legacy",
	treeChangeDiffs: "workspace:tree-change-diffs",
	unapplyStack: "workspace:unapply-stack",
	watcherSubscribe: "workspace:watcher-subscribe",
	watcherUnsubscribe: "workspace:watcher-unsubscribe",
	watcherStopAll: "workspace:watcher-stop-all",
} as const;
