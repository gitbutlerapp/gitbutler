import type { WatcherEvent, AskpassPromptEvent } from "@gitbutler/but-sdk";
import type * as sdk from "@gitbutler/but-sdk";
import type { apiParamNames } from "@gitbutler/but-sdk/api-param-names";
import type { GUISettings } from "./settings.js";

/**
 * What the renderer can call: the exposed endpoints, whose signatures are the
 * SDK's, plus the members electron implements itself.
 */
export type LiteElectronApi = {
	[K in ExposedKey]: EndpointFn<K>;
} & {
	onAskpassPrompt: (callback: (event: AskpassPromptEvent) => void) => () => void;
	askpassSubmitPromptResponse: (params: AskpassSubmitPromptResponseParams) => Promise<void>;
	clipboardWriteText: (text: string) => Promise<void>;
	getVersion: () => Promise<string>;
	isFullScreen: () => Promise<boolean>;
	onFullScreenChange: (callback: (fullScreen: boolean) => void) => () => void;
	openInWebBrowser: (url: string) => Promise<void>;
	pathJoin: (...paths: Array<string>) => Promise<string>;
	showNativeMenu: (params: ShowNativeMenuParams) => Promise<string | null>;
	watcherSubscribe: (projectId: string, callback: (event: WatcherEvent) => void) => Promise<string>;
	watcherUnsubscribe: (subscriptionId: string) => Promise<boolean>;
	watcherStopAll: () => Promise<number>;
	readGUISettings: () => Promise<GUISettings>;
	writeGUISettings: (settings: GUISettings) => Promise<void>;
	platform: string;
};

/**
 * The SDK endpoints the renderer may call. This list is the decision — the
 * signatures, payloads and handlers all follow from it — and an endpoint's
 * name is its IPC channel, so there is nothing else to keep in step.
 */
export const exposedEndpoints = [
	"absorb",
	"absorptionPlan",
	"addCommentReaction",
	"addReviewLabels",
	"addReviewReaction",
	"apply",
	"applyBranchIntegration",
	"assignHunk",
	"branchCheckout",
	"branchCheckoutNew",
	"branchCreate",
	"branchDetails",
	"branchDiff",
	"branchList",
	"branchRemove",
	"branchRename",
	"changesInWorktree",
	"commentArchive",
	"commentCreate",
	"commentUpdate",
	"commentsList",
	"commitAmend",
	"commitCreate",
	"commitDetailsWithLineStats",
	"commitDiscard",
	"commitDiscardChanges",
	"commitInsertBlank",
	"commitMove",
	"commitMoveChangesBetween",
	"commitReword",
	"commitSquash",
	"commitUncommit",
	"commitUncommitChanges",
	"createReviewComment",
	"currentForgeLogin",
	"deleteReviewComment",
	"discardWorktreeChanges",
	"forgeCompareBranchUrl",
	"forgeInfo",
	"forgeProvider",
	"getInitialBranchIntegration",
	"getRedoTargetSnapshot",
	"getRepoInfo",
	"getReview",
	"getReviewBaseRepoUrl",
	"getReviewMergeStatus",
	"getUndoTargetSnapshot",
	"headInfo",
	"listAvailableReviewTemplates",
	"listCiChecks",
	"listCommentReactions",
	"listEditors",
	"listPrograms",
	"listProjectsStateless",
	"listRepoLabels",
	"listReviewComments",
	"listReviewReactions",
	"listReviewSubmissions",
	"listReviewTimelineEvents",
	"listReviewerCandidates",
	"listReviews",
	"listReviewsForBranch",
	"mergeReview",
	"moveBranch",
	"openInProgram",
	"peelRestoreSnapshot",
	"publishReview",
	"removeCommentReaction",
	"removeReviewLabel",
	"removeReviewReaction",
	"requestReview",
	"restoreSnapshotWithKind",
	"reviewTemplate",
	"setReviewAutoMerge",
	"setReviewDraftiness",
	"setReviewTemplate",
	"setTargetRefAndInitProject",
	"tearOffBranch",
	"treeChangeDiffs",
	"unapplyStack",
	"updateReview",
	"updateReviewComment",
	"updateReviewFooters",
	"warmCiChecksCache",
	"withdrawReviewRequest",
	"workspaceBranchAndAncestorsPush",
	"workspaceFetchFromRemotes",
	"workspaceFetchStatus",
	"workspaceIntegrateUpstream",
	"workspaceTargetCommits",
] as const;

/** Members the main process answers itself rather than forwarding to the SDK. */
export const localEndpoints = [
	"askpassPrompt",
	"askpassSubmitPromptResponse",
	"clipboardWriteText",
	"fullScreenChange",
	"getVersion",
	"isFullScreen",
	"openInWebBrowser",
	"pathJoin",
	"readGUISettings",
	"showNativeMenu",
	"watcherStopAll",
	"watcherSubscribe",
	"watcherUnsubscribe",
	"writeGUISettings",
] as const;

// Everything below derives the surface above from the list above.

/** An endpoint the SDK exposes to JavaScript. */
export type Endpoint = keyof typeof apiParamNames & keyof typeof sdk;

/**
 * The payload for an endpoint, named.
 *
 * napi-rs declares endpoints positionally; the SDK separately emits each
 * one's parameter names, generated from the Rust signature that owns them.
 * Pairing the two gives the object shape without restating a single type —
 * every type here still comes from the declaration.
 */
export type PayloadFor<K extends Endpoint> = {
	// Indices only, never the array's own members: mapping straight over
	// `keyof tuple` is homomorphic and would yield an array-like type.
	[I in Extract<keyof (typeof apiParamNames)[K], `${number}`> as (typeof apiParamNames)[K][I] &
		string]: (typeof sdk)[K] extends (...args: infer A) => unknown
		? I extends keyof A
			? A[I]
			: never
		: never;
};

type Result<K extends Endpoint> = ReturnType<(typeof sdk)[K]>;

/** Takes nothing, the lone argument itself, or a payload — by parameter count. */
type EndpointFn<K extends Endpoint> = (typeof apiParamNames)[K]["length"] extends 0
	? () => Result<K>
	: (typeof apiParamNames)[K]["length"] extends 1
		? (arg: Parameters<(typeof sdk)[K]>[0]) => Result<K>
		: (params: PayloadFor<K>) => Result<K>;

export type ExposedKey = (typeof exposedEndpoints)[number];

// Shapes electron owns: no SDK declaration behind them, so they are written
// out by hand — the only types in this file that are.

/** Askpass is not a `#[but_api]` endpoint, so there are no names to derive from. */
export interface AskpassSubmitPromptResponseParams {
	id: string;
	response: string | null;
}

/** Watcher wire shapes; the preload wraps these behind a callback API. */
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
	/** Renders the item as a checkbox in the given state. */
	checked?: boolean;
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
