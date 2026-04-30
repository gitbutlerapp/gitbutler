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
	sourceCommitId: string;
	destinationCommitId: string;
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

export interface PushStackLegacyParams {
	projectId: string;
	stackId: string;
	branch: string;
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

export type NativeCommand = { type: "branches.apply" };

export interface LiteElectronApi {
	absorptionPlan: (params: AbsorptionPlanParams) => Promise<Array<CommitAbsorption>>;
	absorb: (params: AbsorbParams) => Promise<number>;
	apply: (params: ApplyParams) => Promise<ApplyOutcome>;
	assignHunk: (params: AssignHunkParams) => Promise<void>;
	branchDetails: (params: BranchDetailsParams) => Promise<BranchDetails>;
	branchDiff: (params: BranchDiffParams) => Promise<TreeChanges>;
	changesInWorktree: (projectId: string) => Promise<WorktreeChanges>;
	commitAmend: (params: CommitAmendParams) => Promise<CommitCreateResult>;
	commitCreate: (params: CommitCreateParams) => Promise<CommitCreateResult>;
	commitDiscard: (params: CommitDiscardParams) => Promise<CommitDiscardResult>;
	commitDetailsWithLineStats: (params: CommitDetailsWithLineStatsParams) => Promise<CommitDetails>;
	commitInsertBlank: (params: CommitInsertBlankParams) => Promise<CommitInsertBlankResult>;
	commitMove: (params: CommitMoveParams) => Promise<CommitMoveResult>;
	commitSquash: (params: CommitSquashParams) => Promise<CommitSquashResult>;
	commitReword: (params: CommitRewordParams) => Promise<CommitRewordResult>;
	commitMoveChangesBetween: (params: CommitMoveChangesBetweenParams) => Promise<MoveChangesResult>;
	commitUncommitChanges: (params: CommitUncommitChangesParams) => Promise<MoveChangesResult>;
	getVersion: () => Promise<string>;
	headInfo: (projectId: string) => Promise<RefInfo>;
	listBranches: (
		projectId: string,
		filter: BranchListingFilter | null,
	) => Promise<Array<BranchListing>>;
	listProjects: () => Promise<Array<ProjectForFrontend>>;
	moveBranch: (params: MoveBranchParams) => Promise<MoveBranchResult>;
	updateBranchName: (params: UpdateBranchNameParams) => Promise<void>;
	tearOffBranch: (params: TearOffBranchParams) => Promise<MoveBranchResult>;
	ping: (input: string) => Promise<string>;
	pushStackLegacy: (params: PushStackLegacyParams) => Promise<PushResult>;
	showNativeMenu: (params: ShowNativeMenuParams) => Promise<string | null>;
	treeChangeDiffs: (params: TreeChangeDiffParams) => Promise<UnifiedPatch | null>;
	unapplyStack: (params: UnapplyStackParams) => Promise<void>;
	watcherSubscribe: (projectId: string, callback: (event: WatcherEvent) => void) => Promise<string>;
	watcherUnsubscribe: (subscriptionId: string) => Promise<boolean>;
	watcherStopAll: () => Promise<number>;
	onNativeCommand: (callback: (command: NativeCommand) => void) => () => void;
	onUpdateDownloaded: (callback: (info: UpdateDownloadedEvent) => void) => () => void;
	quitAndInstallUpdate: () => Promise<void>;
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
	commitSquash: "workspace:commit-squash",
	commitReword: "workspace:commit-reword",
	commitMoveChangesBetween: "workspace:commit-move-changes-between",
	commitUncommitChanges: "workspace:commit-uncommit-changes",
	getVersion: "lite:get-version",
	headInfo: "workspace:head-info",
	listBranches: "workspace:list-branches",
	listProjects: "projects:list",
	moveBranch: "workspace:move-branch",
	updateBranchName: "workspace:update-branch-name",
	tearOffBranch: "workspace:tear-off-branch",
	ping: "lite:ping",
	pushStackLegacy: "workspace:push-stack-legacy",
	showNativeMenu: "lite:show-native-menu",
	treeChangeDiffs: "workspace:tree-change-diffs",
	unapplyStack: "workspace:unapply-stack",
	watcherSubscribe: "workspace:watcher-subscribe",
	watcherUnsubscribe: "workspace:watcher-unsubscribe",
	watcherStopAll: "workspace:watcher-stop-all",
	nativeCommand: "lite:native-command",
	updaterUpdateDownloaded: "updater:update-downloaded",
	updaterQuitAndInstall: "updater:quit-and-install",
} as const;
