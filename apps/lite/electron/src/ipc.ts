import type {
	AbsorptionTarget,
	ApplyOutcome,
	BranchDetails,
	BranchListing,
	BranchListingFilter,
	BottomUpdate,
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
	CommitCreateResult,
	CommitDiscardResult,
	CommitInsertBlankResult,
	CommitMoveResult,
	CommitRewordResult,
	CommitSquashResult,
	MoveBranchResult,
	MoveChangesResult,
	UnifiedPatch,
	WatcherEvent,
	WorktreeChanges,
	WorkspaceState,
	UncommitResult,
	RestoreKind,
	Snapshot,
} from "@gitbutler/but-sdk";
import type { UpdateDownloadedEvent } from "electron-updater";

export interface AbsorbParams {
	projectId: string;
	absorptionPlan: Array<CommitAbsorption>;
}

export interface AbsorptionPlanParams {
	projectId: string;
	target: AbsorptionTarget;
}

export interface ApplyParams {
	projectId: string;
	existingBranch: string;
}

export interface AssignHunkParams {
	projectId: string;
	assignments: Array<HunkAssignmentRequest>;
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
	dryRun: boolean;
}

export interface CommitCreateParams {
	projectId: string;
	relativeTo: RelativeTo;
	side: InsertSide;
	changes: Array<DiffSpec>;
	message: string;
	dryRun: boolean;
}

export interface CommitDetailsWithLineStatsParams {
	projectId: string;
	commitId: string;
}

export interface CommitDiscardParams {
	projectId: string;
	subjectCommitId: string;
	dryRun: boolean;
}

export interface CommitInsertBlankParams {
	projectId: string;
	relativeTo: RelativeTo;
	side: InsertSide;
	dryRun: boolean;
}

export interface CommitMoveParams {
	projectId: string;
	subjectCommitIds: Array<string>;
	relativeTo: RelativeTo;
	side: InsertSide;
	dryRun: boolean;
}

export interface CommitMoveChangesBetweenParams {
	projectId: string;
	sourceCommitId: string;
	destinationCommitId: string;
	changes: Array<DiffSpec>;
	dryRun: boolean;
}

export interface CommitRewordParams {
	projectId: string;
	commitId: string;
	message: string;
	dryRun: boolean;
}

export interface CommitSquashParams {
	projectId: string;
	sourceCommitIds: Array<string>;
	destinationCommitId: string;
	dryRun: boolean;
}

export interface CommitUncommitParams {
	projectId: string;
	subjectCommitIds: Array<string>;
	assignTo: string | null;
	dryRun: boolean;
}

export interface CommitUncommitChangesParams {
	projectId: string;
	commitId: string;
	changes: Array<DiffSpec>;
	assignTo: string | null;
	dryRun: boolean;
}

export interface ListBranchesParams {
	projectId: string;
	filter: BranchListingFilter | null;
}

export interface MoveBranchParams {
	projectId: string;
	subjectBranch: string;
	targetBranch: string;
	dryRun: boolean;
}

export interface PeelRestoreSnapshotParams {
	projectId: string;
	sha: string;
}

export interface PushStackParams {
	projectId: string;
	stackId: string;
	branch: string;
}

export interface RemoveBranchParams {
	projectId: string;
	stackId: string;
	branchName: string;
}

export interface RestoreSnapshotWithKindParams {
	projectId: string;
	restoreKind: RestoreKind;
	sha: string;
}

export interface TearOffBranchParams {
	projectId: string;
	subjectBranch: string;
	dryRun: boolean;
}

export interface TreeChangeDiffParams {
	projectId: string;
	change: TreeChange;
}

export interface UnapplyStackParams {
	projectId: string;
	stackId: string;
}

export interface WorkspaceIntegrateUpstreamParams {
	projectId: string;
	updates: Array<BottomUpdate>;
	dryRun: boolean;
}

export interface UpdateBranchNameParams {
	projectId: string;
	stackId: string;
	branchName: string;
	newName: string;
}

export interface WatcherSubscribeParams {
	projectId: string;
}

export interface WatcherSubscribeResult {
	subscriptionId: string;
	eventChannel: string;
}

export interface WatcherUnsubscribeParams {
	subscriptionId: string;
}

export interface NativeMenuPosition {
	x: number;
	y: number;
}

type NativeMenuPopupItemData = {
	label: string;
	accelerator?: string;
	enabled?: boolean;
	itemId?: string;
	submenu?: Array<NativeMenuPopupItem>;
};

export type NativeMenuPopupItem =
	| { _tag: "Separator" }
	| ({ _tag: "Item" } & NativeMenuPopupItemData);

export interface ShowNativeMenuParams {
	items: Array<NativeMenuPopupItem>;
	position: NativeMenuPosition;
}

export interface LiteElectronApi {
	absorptionPlan: (params: AbsorptionPlanParams) => Promise<Array<CommitAbsorption>>;
	absorb: (params: AbsorbParams) => Promise<number>;
	apply: (params: ApplyParams) => Promise<ApplyOutcome>;
	assignHunk: (params: AssignHunkParams) => Promise<void>;
	branchDetails: (params: BranchDetailsParams) => Promise<BranchDetails>;
	branchDiff: (params: BranchDiffParams) => Promise<TreeChanges>;
	changesInWorktree: (projectId: string) => Promise<WorktreeChanges>;
	clipboardWriteText: (text: string) => Promise<void>;
	commitAmend: (params: CommitAmendParams) => Promise<CommitCreateResult>;
	commitCreate: (params: CommitCreateParams) => Promise<CommitCreateResult>;
	commitDiscard: (params: CommitDiscardParams) => Promise<CommitDiscardResult>;
	commitDetailsWithLineStats: (params: CommitDetailsWithLineStatsParams) => Promise<CommitDetails>;
	commitInsertBlank: (params: CommitInsertBlankParams) => Promise<CommitInsertBlankResult>;
	commitMove: (params: CommitMoveParams) => Promise<CommitMoveResult>;
	commitSquash: (params: CommitSquashParams) => Promise<CommitSquashResult>;
	commitReword: (params: CommitRewordParams) => Promise<CommitRewordResult>;
	commitMoveChangesBetween: (params: CommitMoveChangesBetweenParams) => Promise<MoveChangesResult>;
	commitUncommit: (params: CommitUncommitParams) => Promise<UncommitResult>;
	commitUncommitChanges: (params: CommitUncommitChangesParams) => Promise<MoveChangesResult>;
	getVersion: () => Promise<string>;
	getRedoTargetSnapshot: (projectId: string) => Promise<Snapshot | null>;
	getUndoTargetSnapshot: (projectId: string) => Promise<Snapshot | null>;
	headInfo: (projectId: string) => Promise<RefInfo>;
	listBranches: (
		projectId: string,
		filter: BranchListingFilter | null,
	) => Promise<Array<BranchListing>>;
	listProjects: () => Promise<Array<ProjectForFrontend>>;
	moveBranch: (params: MoveBranchParams) => Promise<MoveBranchResult>;
	pathJoin: (...paths: Array<string>) => Promise<string>;
	updateBranchName: (params: UpdateBranchNameParams) => Promise<void>;
	tearOffBranch: (params: TearOffBranchParams) => Promise<MoveBranchResult>;
	peelRestoreSnapshot: (params: PeelRestoreSnapshotParams) => Promise<Snapshot | null>;
	pushStack: (params: PushStackParams) => Promise<PushResult>;
	removeBranch: (params: RemoveBranchParams) => Promise<void>;
	restoreSnapshotWithKind: (params: RestoreSnapshotWithKindParams) => Promise<void>;
	showNativeMenu: (params: ShowNativeMenuParams) => Promise<string | null>;
	treeChangeDiffs: (params: TreeChangeDiffParams) => Promise<UnifiedPatch | null>;
	unapplyStack: (params: UnapplyStackParams) => Promise<void>;
	workspaceIntegrateUpstream: (params: WorkspaceIntegrateUpstreamParams) => Promise<WorkspaceState>;
	watcherSubscribe: (projectId: string, callback: (event: WatcherEvent) => void) => Promise<string>;
	watcherUnsubscribe: (subscriptionId: string) => Promise<boolean>;
	watcherStopAll: () => Promise<number>;
	onUpdateDownloaded: (callback: (info: UpdateDownloadedEvent) => void) => () => void;
	quitAndInstallUpdate: () => Promise<void>;
	platform: string;
}

export const liteIpcChannels = {
	absorptionPlan: "workspace:absorption-plan",
	absorb: "workspace:absorb",
	apply: "workspace:apply",
	assignHunk: "workspace:assign-hunk",
	branchDetails: "workspace:branch-details",
	branchDiff: "workspace:branch-diff",
	changesInWorktree: "workspace:changes-in-worktree",
	clipboardWriteText: "lite:clipboard-write-text",
	commitAmend: "workspace:commit-amend",
	commitCreate: "workspace:commit-create",
	commitDiscard: "workspace:commit-discard",
	commitDetailsWithLineStats: "workspace:commit-details-with-line-stats",
	commitInsertBlank: "workspace:commit-insert-blank",
	commitMove: "workspace:commit-move",
	commitSquash: "workspace:commit-squash",
	commitReword: "workspace:commit-reword",
	commitMoveChangesBetween: "workspace:commit-move-changes-between",
	commitUncommit: "workspace:commit-uncommit",
	commitUncommitChanges: "workspace:commit-uncommit-changes",
	getRedoTargetSnapshot: "workspace:get-redo-target-snapshot",
	getUndoTargetSnapshot: "workspace:get-undo-target-snapshot",
	getVersion: "lite:get-version",
	headInfo: "workspace:head-info",
	listBranches: "workspace:list-branches",
	listProjects: "projects:list",
	moveBranch: "workspace:move-branch",
	pathJoin: "lite:path-join",
	updateBranchName: "workspace:update-branch-name",
	tearOffBranch: "workspace:tear-off-branch",
	peelRestoreSnapshot: "workspace:peel-restore-snapshot",
	pushStack: "workspace:push-stack",
	removeBranch: "workspace:remove-branch",
	restoreSnapshotWithKind: "workspace:restore-snapshot-with-kind",
	showNativeMenu: "lite:show-native-menu",
	treeChangeDiffs: "workspace:tree-change-diffs",
	unapplyStack: "workspace:unapply-stack",
	workspaceIntegrateUpstream: "workspace:integrate-upstream",
	watcherSubscribe: "workspace:watcher-subscribe",
	watcherUnsubscribe: "workspace:watcher-unsubscribe",
	watcherStopAll: "workspace:watcher-stop-all",
	updaterUpdateDownloaded: "updater:update-downloaded",
	updaterQuitAndInstall: "updater:quit-and-install",
} as const;
