import type {
	AbsorptionTarget,
	ApplyOutcome,
	BranchCheckoutResult,
	BranchIntegrationStrategy,
	BranchReference,
	BranchCreatePlacement,
	BranchCreateResult,
	BranchDetails,
	BranchListing,
	BranchListingFilter,
	BottomUpdate,
	CacheConfig,
	CiCheck,
	Editor,
	ForgeInfo,
	ForgeName,
	ForgeReview,
	ForgeReviewFilter,
	ForgeReviewUpdate,
	CommitAbsorption,
	HunkAssignmentRequest,
	CommitDetails,
	DiffSpec,
	FullNameBytes,
	InitialBranchIntegration,
	InsertSide,
	IntegrateBranchResult,
	InteractiveIntegration,
	MessageCombinationStrategy,
	PushResult,
	ProjectForFrontend,
	RelativeTo,
	RefInfo,
	RepoInfo,
	TreeChange,
	TreeChanges,
	CommitCreateResult,
	CommitDiscardResult,
	CommitInsertBlankResult,
	CommitMoveResult,
	CommitRewordResult,
	CommitSquashResult,
	CreateForgeReviewParams,
	MoveBranchResult,
	MoveChangesResult,
	PushFlag,
	UnifiedPatch,
	WatcherEvent,
	WorktreeChanges,
	WorkspaceState,
	UncommitResult,
	ReviewState,
	ReviewMergeMethod,
	ReviewMergeStatus,
	ReviewTemplateInfo,
	RestoreKind,
	Snapshot,
	AskpassPromptEvent,
	MaybeLossyFullNameRef,
} from "@gitbutler/but-sdk";

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

export interface ApplyBranchIntegrationParams {
	projectId: string;
	branch: string;
	integration: InteractiveIntegration;
	dryRun: boolean;
}

export interface AskpassSubmitPromptResponseParams {
	id: string;
	response: string | null;
}

export interface AssignHunkParams {
	projectId: string;
	assignments: Array<HunkAssignmentRequest>;
}

export interface BranchCreateParams {
	projectId: string;
	newRef: MaybeLossyFullNameRef;
	placement: BranchCreatePlacement;
}

export interface BranchCheckoutParams {
	projectId: string;
	branch: FullNameBytes;
}

export interface BranchCheckoutNewParams {
	projectId: string;
	name: string | null;
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

export interface CommitDiscardChangesParams {
	projectId: string;
	commitId: string;
	changes: Array<DiffSpec>;
	dryRun: boolean;
}

export interface DiscardWorktreeChangesParams {
	projectId: string;
	changes: Array<DiffSpec>;
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
	howToCombineMessages?: MessageCombinationStrategy;
	dryRun: boolean;
}

export interface ForgeCompareBranchUrlParams {
	projectId: string;
	base: string;
	branch: string;
	fork: string | null;
}

export interface GetInitialBranchIntegrationParams {
	projectId: string;
	branch: string;
	strategy: BranchIntegrationStrategy | null;
}

export interface GetReviewBaseRepoUrlParams {
	projectId: string;
	reviewId: number;
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

export interface ListReviewsForBranchParams {
	projectId: string;
	branch: string;
	filter: ForgeReviewFilter | null;
}

export interface GetReviewParams {
	projectId: string;
	reviewId: number;
}

export interface ListCiChecksParams {
	projectId: string;
	reference: string;
	cacheConfig: CacheConfig | null;
}

export interface ListReviewsParams {
	projectId: string;
	cacheConfig: CacheConfig | null;
}

export interface MoveBranchParams {
	projectId: string;
	subjectBranch: string;
	targetBranch: string;
	dryRun: boolean;
}

export interface MergeReviewParams {
	projectId: string;
	reviewId: number;
	mergeMethod: ReviewMergeMethod | null;
}

export interface OpenInEditorParams {
	projectId: string;
	editorId: string;
	path: string;
	lineNr: number | null;
}

export interface PeelRestoreSnapshotParams {
	projectId: string;
	sha: string;
}

export interface PublishReviewParams {
	projectId: string;
	params: CreateForgeReviewParams;
}

export interface PushStackParams {
	projectId: string;
	branch: string;
	withForce: boolean;
	skipForcePushProtection: boolean;
	runHooks: boolean;
	pushOpts: Array<PushFlag>;
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

export interface SetReviewAutoMergeParams {
	projectId: string;
	reviewId: number;
	enable: boolean;
}

export interface SetReviewDraftinessParams {
	projectId: string;
	reviewId: number;
	draft: boolean;
}

export interface SetReviewTemplateParams {
	projectId: string;
	templatePath: string | null;
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

export interface UpdateReviewParams {
	projectId: string;
	reviewId: number;
	title: string | null;
	body: string | null;
	state: ReviewState | null;
	targetBase: string | null;
}

export interface UpdateReviewFootersParams {
	projectId: string;
	reviews: Array<ForgeReviewUpdate>;
}

export type UpdateBranchNameResult = BranchReference;

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
	applyBranchIntegration: (params: ApplyBranchIntegrationParams) => Promise<IntegrateBranchResult>;
	onAskpassPrompt: (callback: (event: AskpassPromptEvent) => void) => () => void;
	askpassSubmitPromptResponse: (params: AskpassSubmitPromptResponseParams) => Promise<void>;
	assignHunk: (params: AssignHunkParams) => Promise<void>;
	branchCheckout: (params: BranchCheckoutParams) => Promise<BranchCheckoutResult>;
	branchCheckoutNew: (params: BranchCheckoutNewParams) => Promise<BranchCheckoutResult>;
	branchCreate: (params: BranchCreateParams) => Promise<BranchCreateResult>;
	branchDetails: (params: BranchDetailsParams) => Promise<BranchDetails>;
	branchDiff: (params: BranchDiffParams) => Promise<TreeChanges>;
	changesInWorktree: (projectId: string) => Promise<WorktreeChanges>;
	clipboardWriteText: (text: string) => Promise<void>;
	commitAmend: (params: CommitAmendParams) => Promise<CommitCreateResult>;
	commitCreate: (params: CommitCreateParams) => Promise<CommitCreateResult>;
	commitDiscard: (params: CommitDiscardParams) => Promise<CommitDiscardResult>;
	commitDiscardChanges: (params: CommitDiscardChangesParams) => Promise<MoveChangesResult>;
	commitDetailsWithLineStats: (params: CommitDetailsWithLineStatsParams) => Promise<CommitDetails>;
	discardWorktreeChanges: (params: DiscardWorktreeChangesParams) => Promise<Array<DiffSpec>>;
	commitInsertBlank: (params: CommitInsertBlankParams) => Promise<CommitInsertBlankResult>;
	commitMove: (params: CommitMoveParams) => Promise<CommitMoveResult>;
	commitSquash: (params: CommitSquashParams) => Promise<CommitSquashResult>;
	commitReword: (params: CommitRewordParams) => Promise<CommitRewordResult>;
	commitMoveChangesBetween: (params: CommitMoveChangesBetweenParams) => Promise<MoveChangesResult>;
	commitUncommit: (params: CommitUncommitParams) => Promise<UncommitResult>;
	commitUncommitChanges: (params: CommitUncommitChangesParams) => Promise<MoveChangesResult>;
	forgeCompareBranchUrl: (params: ForgeCompareBranchUrlParams) => Promise<string | null>;
	forgeInfo: (projectId: string) => Promise<ForgeInfo | null>;
	forgeProvider: (projectId: string) => Promise<ForgeName | null>;
	getInitialBranchIntegration: (
		params: GetInitialBranchIntegrationParams,
	) => Promise<InitialBranchIntegration>;
	getRepoInfo: (projectId: string) => Promise<RepoInfo>;
	getReviewBaseRepoUrl: (params: GetReviewBaseRepoUrlParams) => Promise<string | null>;
	getReviewMergeStatus: (params: GetReviewParams) => Promise<ReviewMergeStatus>;
	getVersion: () => Promise<string>;
	getRedoTargetSnapshot: (projectId: string) => Promise<Snapshot | null>;
	getReview: (params: GetReviewParams) => Promise<ForgeReview>;
	getUndoTargetSnapshot: (projectId: string) => Promise<Snapshot | null>;
	headInfo: (projectId: string) => Promise<RefInfo>;
	listBranches: (
		projectId: string,
		filter: BranchListingFilter | null,
	) => Promise<Array<BranchListing>>;
	listAvailableReviewTemplates: (projectId: string) => Promise<Array<string>>;
	listCiChecks: (params: ListCiChecksParams) => Promise<Array<CiCheck>>;
	listEditors: () => Promise<Array<Editor>>;
	listProjectsStateless: () => Promise<Array<ProjectForFrontend>>;
	listReviews: (params: ListReviewsParams) => Promise<Array<ForgeReview>>;
	listReviewsForBranch: (params: ListReviewsForBranchParams) => Promise<Array<ForgeReview>>;
	mergeReview: (params: MergeReviewParams) => Promise<void>;
	moveBranch: (params: MoveBranchParams) => Promise<MoveBranchResult>;
	openInEditor: (params: OpenInEditorParams) => Promise<void>;
	pathJoin: (...paths: Array<string>) => Promise<string>;
	publishReview: (params: PublishReviewParams) => Promise<ForgeReview>;
	updateBranchName: (params: UpdateBranchNameParams) => Promise<UpdateBranchNameResult>;
	updateReview: (params: UpdateReviewParams) => Promise<void>;
	tearOffBranch: (params: TearOffBranchParams) => Promise<MoveBranchResult>;
	peelRestoreSnapshot: (params: PeelRestoreSnapshotParams) => Promise<Snapshot | null>;
	pushStack: (params: PushStackParams) => Promise<PushResult>;
	removeBranch: (params: RemoveBranchParams) => Promise<void>;
	restoreSnapshotWithKind: (params: RestoreSnapshotWithKindParams) => Promise<void>;
	reviewTemplate: (projectId: string) => Promise<ReviewTemplateInfo | null>;
	setReviewAutoMerge: (params: SetReviewAutoMergeParams) => Promise<void>;
	setReviewDraftiness: (params: SetReviewDraftinessParams) => Promise<void>;
	setReviewTemplate: (params: SetReviewTemplateParams) => Promise<void>;
	showNativeMenu: (params: ShowNativeMenuParams) => Promise<string | null>;
	treeChangeDiffs: (params: TreeChangeDiffParams) => Promise<UnifiedPatch | null>;
	unapplyStack: (params: UnapplyStackParams) => Promise<void>;
	workspaceIntegrateUpstream: (params: WorkspaceIntegrateUpstreamParams) => Promise<WorkspaceState>;
	updateReviewFooters: (params: UpdateReviewFootersParams) => Promise<void>;
	warmCiChecksCache: (projectId: string) => Promise<void>;
	watcherSubscribe: (projectId: string, callback: (event: WatcherEvent) => void) => Promise<string>;
	watcherUnsubscribe: (subscriptionId: string) => Promise<boolean>;
	watcherStopAll: () => Promise<number>;
	platform: string;
}

export const liteIpcChannels = {
	absorptionPlan: "workspace:absorption-plan",
	absorb: "workspace:absorb",
	apply: "workspace:apply",
	applyBranchIntegration: "workspace:apply-branch-integration",
	askpassPrompt: "askpass:prompt",
	askpassSubmitPromptResponse: "askpass:submit-prompt-response",
	assignHunk: "workspace:assign-hunk",
	branchCheckout: "workspace:branch-checkout",
	branchCheckoutNew: "workspace:branch-checkout-new",
	branchCreate: "workspace:branch-create",
	branchDetails: "workspace:branch-details",
	branchDiff: "workspace:branch-diff",
	changesInWorktree: "workspace:changes-in-worktree",
	clipboardWriteText: "lite:clipboard-write-text",
	commitAmend: "workspace:commit-amend",
	commitCreate: "workspace:commit-create",
	commitDiscard: "workspace:commit-discard",
	commitDiscardChanges: "workspace:commit-discard-changes",
	commitDetailsWithLineStats: "workspace:commit-details-with-line-stats",
	discardWorktreeChanges: "workspace:discard-worktree-changes",
	commitInsertBlank: "workspace:commit-insert-blank",
	commitMove: "workspace:commit-move",
	commitSquash: "workspace:commit-squash",
	commitReword: "workspace:commit-reword",
	commitMoveChangesBetween: "workspace:commit-move-changes-between",
	commitUncommit: "workspace:commit-uncommit",
	commitUncommitChanges: "workspace:commit-uncommit-changes",
	forgeCompareBranchUrl: "workspace:forge-compare-branch-url",
	forgeInfo: "workspace:forge-info",
	forgeProvider: "workspace:forge-provider",
	getInitialBranchIntegration: "workspace:get-initial-branch-integration",
	getRepoInfo: "workspace:get-repo-info",
	getReviewBaseRepoUrl: "workspace:get-review-base-repo-url",
	getReviewMergeStatus: "workspace:get-review-merge-status",
	getVersion: "lite:get-version",
	getRedoTargetSnapshot: "workspace:get-redo-target-snapshot",
	getReview: "workspace:get-review",
	getUndoTargetSnapshot: "workspace:get-undo-target-snapshot",
	headInfo: "workspace:head-info",
	listBranches: "workspace:list-branches",
	listAvailableReviewTemplates: "workspace:list-available-review-templates",
	listCiChecks: "workspace:list-ci-checks",
	listEditors: "workspace:list-editors",
	listProjectsStateless: "projects:list-stateless",
	listReviews: "workspace:list-reviews",
	listReviewsForBranch: "workspace:list-reviews-for-branch",
	mergeReview: "workspace:merge-review",
	moveBranch: "workspace:move-branch",
	openInEditor: "workspace:open-in-editor",
	pathJoin: "lite:path-join",
	publishReview: "workspace:publish-review",
	updateBranchName: "workspace:update-branch-name",
	updateReview: "workspace:update-review",
	tearOffBranch: "workspace:tear-off-branch",
	peelRestoreSnapshot: "workspace:peel-restore-snapshot",
	pushStack: "workspace:push-stack",
	removeBranch: "workspace:remove-branch",
	restoreSnapshotWithKind: "workspace:restore-snapshot-with-kind",
	reviewTemplate: "workspace:review-template",
	setReviewAutoMerge: "workspace:set-review-auto-merge",
	setReviewDraftiness: "workspace:set-review-draftiness",
	setReviewTemplate: "workspace:set-review-template",
	showNativeMenu: "lite:show-native-menu",
	treeChangeDiffs: "workspace:tree-change-diffs",
	unapplyStack: "workspace:unapply-stack",
	workspaceIntegrateUpstream: "workspace:integrate-upstream",
	updateReviewFooters: "workspace:update-review-footers",
	warmCiChecksCache: "workspace:warm-ci-checks-cache",
	watcherSubscribe: "workspace:watcher-subscribe",
	watcherUnsubscribe: "workspace:watcher-unsubscribe",
	watcherStopAll: "workspace:watcher-stop-all",
} as const;
