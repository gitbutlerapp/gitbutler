import { contextBridge, ipcRenderer } from "electron";
import type { LiteElectronApi, UpdateBranchNameResult, WatcherSubscribeResult } from "./ipc";
import type {
	CommitAbsorption,
	ApplyOutcome,
	BranchCheckoutResult,
	BranchCreateResult,
	BranchDetails,
	BranchListing,
	CommitDetails,
	DiffSpec,
	Editor,
	ForgeReview,
	ProjectForFrontend,
	PushResult,
	RefInfo,
	TreeChanges,
	CommitCreateResult,
	CommitDiscardResult,
	CommitInsertBlankResult,
	CommitMoveResult,
	CommitRewordResult,
	CommitSquashResult,
	CiCheck,
	ForgeInfo,
	ForgeName,
	MoveBranchResult,
	MoveChangesResult,
	InitialBranchIntegration,
	IntegrateBranchResult,
	UnifiedPatch,
	WatcherEvent,
	WorktreeChanges,
	WorkspaceIntegrateUpstreamOutcome,
	UncommitResult,
	Snapshot,
	AskpassPromptEvent,
	RepoInfo,
	ReviewMergeStatus,
	ReviewTemplateInfo,
} from "@gitbutler/but-sdk";
import type { GUISettings } from "./settings";

// Sandboxed preloads cannot load local modules. Keep these values in sync with tracing.ts.
const ipcTraceCompleteChannel = "lite:ipc-trace-complete";
const ipcTraceWatcherEventChannel = "lite:ipc-trace-watcher-event";
const ipcTraceWatcherHost = "ipc-watcher.localhost";
const ipcTracePathPrefix = "/__ipc/";
const ipcTraceAcceptPrefix = "application/json; trace-id=";
const ipcTraceStreamAcceptPrefix = "text/event-stream; trace-id=";
const ipcTraceWatcherEventsPath = `${ipcTracePathPrefix}watcher-events`;
const maxTraceResponseCharacters = 1_000_000;
const maxTraceArgsCharacters = 4_000;
const maxTraceGateMs = 60_000;
const ipcTraceServerUrl = (() => {
	const devServerUrl = process.env.VITE_DEV_SERVER_URL;
	if (devServerUrl === undefined) return undefined;

	const url = new URL(devServerUrl);
	url.hostname = "ipc.localhost";
	return url;
})();
const ipcTraceWatcherServerUrl = (() => {
	if (ipcTraceServerUrl === undefined) return undefined;

	const url = new URL(ipcTraceServerUrl);
	url.hostname = ipcTraceWatcherHost;
	return url;
})();

interface IpcTrace {
	traceId: string;
	startedAt: number;
	finished: Promise<void>;
	stopWaiting: () => void;
}

const traceSafeArgs = (channel: string, args: Array<unknown>): Array<unknown> => {
	if (channel === "askpass:submit-prompt-response") {
		const [params, ...rest] = args;
		if (typeof params === "object" && params !== null)
			return [{ ...params, response: "<redacted>" }, ...rest];
	}

	if (channel === "lite:clipboard-write-text") return ["<redacted>"];

	return args;
};

// Lite's IPC arguments and results are JSON-compatible. Errors are the one useful exception: their
// properties are not enumerable, so normalize them before displaying them in DevTools.
const stringifyTracePayload = (
	value: unknown,
	maxCharacters = maxTraceResponseCharacters,
): string => {
	let json: string;

	try {
		json = JSON.stringify(value, (_key, nestedValue: unknown) => {
			if (nestedValue instanceof Error) {
				return {
					name: nestedValue.name,
					message: nestedValue.message,
					stack: nestedValue.stack,
				};
			}
			return nestedValue;
		});
	} catch (error) {
		json = JSON.stringify({ serializationError: String(error) });
	}

	if (json.length <= maxCharacters) return json;
	return JSON.stringify({
		truncated: true,
		originalCharacters: json.length,
		preview: json.slice(0, maxCharacters),
	});
};

const beginIpcTrace = (channel: string, args: Array<unknown>): IpcTrace | undefined => {
	if (ipcTraceServerUrl === undefined) return undefined;

	const traceId = crypto.randomUUID();
	const separatorIndex = channel.indexOf(":");
	const scope = separatorIndex === -1 ? "ipc" : channel.slice(0, separatorIndex);
	const method = separatorIndex === -1 ? channel : channel.slice(separatorIndex + 1);
	const traceUrl = new URL(
		`${ipcTracePathPrefix}${encodeURIComponent(scope)}/${encodeURIComponent(method)}`,
		ipcTraceServerUrl,
	);
	const serializedArgs = stringifyTracePayload(
		traceSafeArgs(channel, args),
		maxTraceArgsCharacters,
	);
	const abortController = new AbortController();
	const startedAt = performance.now();

	const response = fetch(traceUrl, {
		method: "POST",
		headers: { Accept: `${ipcTraceAcceptPrefix}${traceId}` },
		body: serializedArgs,
		signal: abortController.signal,
	});
	const timeout = setTimeout(() => abortController.abort(), maxTraceGateMs);
	const finished = response
		.then((response) => response.arrayBuffer())
		.then(() => undefined)
		.catch(() => undefined)
		.finally(() => clearTimeout(timeout));

	return {
		traceId,
		startedAt,
		finished,
		stopWaiting: () => abortController.abort(),
	};
};

const completeIpcTrace = async (
	trace: IpcTrace | undefined,
	ok: boolean,
	value: unknown,
): Promise<void> => {
	if (trace === undefined) return;

	try {
		const stored = (await ipcRenderer.invoke(ipcTraceCompleteChannel, {
			traceId: trace.traceId,
			ok,
			body: stringifyTracePayload(value),
			durationMs: performance.now() - trace.startedAt,
		})) as unknown;
		if (stored !== true) {
			trace.stopWaiting();
			return;
		}

		await trace.finished;
	} catch {
		trace.stopWaiting();
		// Tracing must never affect the real IPC call.
	}
};

const invoke = async (channel: string, ...args: Array<unknown>): Promise<unknown> => {
	const trace = beginIpcTrace(channel, args);

	try {
		const result = (await ipcRenderer.invoke(channel, ...args)) as unknown;
		await completeIpcTrace(trace, true, result);
		return result;
	} catch (error) {
		await completeIpcTrace(trace, false, error);
		throw error;
	}
};

interface IpcWatcherTraceStream {
	streamId: string;
	connected: boolean;
	active: boolean;
	stop: () => void;
}

let watcherTraceStream: IpcWatcherTraceStream | undefined;

const startWatcherTraceStream = (): void => {
	if (ipcTraceWatcherServerUrl === undefined || watcherTraceStream?.active === true) return;

	const streamId = crypto.randomUUID();
	const abortController = new AbortController();
	const stream: IpcWatcherTraceStream = {
		streamId,
		connected: false,
		active: true,
		stop: () => {
			stream.active = false;
			abortController.abort();
		},
	};
	watcherTraceStream = stream;

	const streamUrl = new URL(ipcTraceWatcherEventsPath, ipcTraceWatcherServerUrl);
	void fetch(streamUrl, {
		headers: { Accept: `${ipcTraceStreamAcceptPrefix}${streamId}` },
		signal: abortController.signal,
	})
		.then(async (response) => {
			if (!response.ok || !stream.active) return;
			stream.connected = true;

			// Drain incrementally: response.text() would retain the stream's entire lifetime in memory.
			const reader = response.body?.getReader();
			if (reader === undefined) return;
			while (!(await reader.read()).done) {
				// DevTools records the server-sent events; preload only needs to consume the bytes.
			}
		})
		.catch(() => undefined)
		.finally(() => {
			stream.connected = false;
			stream.active = false;
		});
};

const stopWatcherTraceStream = (): void => {
	watcherTraceStream?.stop();
	watcherTraceStream = undefined;
};

const traceWatcherEvent = (
	projectId: string,
	subscriptionId: string,
	event: WatcherEvent,
): void => {
	const stream = watcherTraceStream;
	if (stream?.connected !== true) return;

	void ipcRenderer
		.invoke(ipcTraceWatcherEventChannel, {
			streamId: stream.streamId,
			type: event.payload.type,
			body: stringifyTracePayload({
				projectId,
				subscriptionId,
				payload: event.payload,
			}),
		})
		.catch(() => undefined);
};

/**
 * The map of subscription IDs to channels and callbacks.
 *
 * This is needed in order to maintain separate changes for each subscription.
 * The subscription ID is known to the UI, but the channel is not.
 */
const watcherListenerBySubscription = new Map<
	string,
	{
		eventChannel: string;
		listener: (_event: Electron.IpcRendererEvent, payload: WatcherEvent) => void;
	}
>();

const api: LiteElectronApi = {
	absorptionPlan: (params) =>
		invoke("workspace:absorption-plan", params) as Promise<Array<CommitAbsorption>>,
	absorb: (params) => invoke("workspace:absorb", params) as Promise<number>,
	apply: (params) => invoke("workspace:apply", params) as Promise<ApplyOutcome>,
	applyBranchIntegration: (params) =>
		invoke("workspace:apply-branch-integration", params) as Promise<IntegrateBranchResult>,
	onAskpassPrompt: (callback) => {
		const listener = (_event: Electron.IpcRendererEvent, payload: AskpassPromptEvent) => {
			callback(payload);
		};
		ipcRenderer.on("askpass:prompt", listener);
		return () => ipcRenderer.removeListener("askpass:prompt", listener);
	},
	askpassSubmitPromptResponse: (params) =>
		invoke("askpass:submit-prompt-response", params) as Promise<void>,
	assignHunk: (params) => invoke("workspace:assign-hunk", params) as Promise<void>,
	branchCheckout: (params) =>
		invoke("workspace:branch-checkout", params) as Promise<BranchCheckoutResult>,
	branchCheckoutNew: (params) =>
		invoke("workspace:branch-checkout-new", params) as Promise<BranchCheckoutResult>,
	branchCreate: (params) =>
		invoke("workspace:branch-create", params) as Promise<BranchCreateResult>,
	branchDetails: (params) => invoke("workspace:branch-details", params) as Promise<BranchDetails>,
	branchDiff: (params) => invoke("workspace:branch-diff", params) as Promise<TreeChanges>,
	changesInWorktree: (projectId) =>
		invoke("workspace:changes-in-worktree", projectId) as Promise<WorktreeChanges>,
	clipboardWriteText: (text) => invoke("lite:clipboard-write-text", text) as Promise<void>,
	commitAmend: (params) => invoke("workspace:commit-amend", params) as Promise<CommitCreateResult>,
	commitCreate: (params) =>
		invoke("workspace:commit-create", params) as Promise<CommitCreateResult>,
	commitDiscard: (params) =>
		invoke("workspace:commit-discard", params) as Promise<CommitDiscardResult>,
	commitDiscardChanges: (params) =>
		invoke("workspace:commit-discard-changes", params) as Promise<MoveChangesResult>,
	commitDetailsWithLineStats: (params) =>
		invoke("workspace:commit-details-with-line-stats", params) as Promise<CommitDetails>,
	discardWorktreeChanges: (params) =>
		invoke("workspace:discard-worktree-changes", params) as Promise<Array<DiffSpec>>,
	commitInsertBlank: (params) =>
		invoke("workspace:commit-insert-blank", params) as Promise<CommitInsertBlankResult>,
	commitMove: (params) => invoke("workspace:commit-move", params) as Promise<CommitMoveResult>,
	commitSquash: (params) =>
		invoke("workspace:commit-squash", params) as Promise<CommitSquashResult>,
	commitReword: (params) =>
		invoke("workspace:commit-reword", params) as Promise<CommitRewordResult>,
	commitMoveChangesBetween: (params) =>
		invoke("workspace:commit-move-changes-between", params) as Promise<MoveChangesResult>,
	commitUncommit: (params) =>
		invoke("workspace:commit-uncommit", params) as Promise<UncommitResult>,
	commitUncommitChanges: (params) =>
		invoke("workspace:commit-uncommit-changes", params) as Promise<MoveChangesResult>,
	forgeCompareBranchUrl: (params) =>
		invoke("workspace:forge-compare-branch-url", params) as Promise<string | null>,
	forgeInfo: (projectId) => invoke("workspace:forge-info", projectId) as Promise<ForgeInfo | null>,
	forgeProvider: (projectId) =>
		invoke("workspace:forge-provider", projectId) as Promise<ForgeName | null>,
	getInitialBranchIntegration: (params) =>
		invoke("workspace:get-initial-branch-integration", params) as Promise<InitialBranchIntegration>,
	getRepoInfo: (projectId) => invoke("workspace:get-repo-info", projectId) as Promise<RepoInfo>,
	getReviewBaseRepoUrl: (params) =>
		invoke("workspace:get-review-base-repo-url", params) as Promise<string | null>,
	getReviewMergeStatus: (params) =>
		invoke("workspace:get-review-merge-status", params) as Promise<ReviewMergeStatus>,
	getVersion: () => invoke("lite:get-version") as Promise<string>,
	getRedoTargetSnapshot: (params) =>
		invoke("workspace:get-redo-target-snapshot", params) as Promise<Snapshot | null>,
	getReview: (params) => invoke("workspace:get-review", params) as Promise<ForgeReview>,
	getUndoTargetSnapshot: (params) =>
		invoke("workspace:get-undo-target-snapshot", params) as Promise<Snapshot | null>,
	headInfo: (projectId) => invoke("workspace:head-info", projectId) as Promise<RefInfo>,
	listBranches: (projectId, filter) =>
		invoke("workspace:list-branches", projectId, filter) as Promise<Array<BranchListing>>,
	listAvailableReviewTemplates: (projectId) =>
		invoke("workspace:list-available-review-templates", projectId) as Promise<Array<string>>,
	listCiChecks: (params) => invoke("workspace:list-ci-checks", params) as Promise<Array<CiCheck>>,
	listEditors: () => invoke("workspace:list-editors") as Promise<Array<Editor>>,
	listProjectsStateless: () =>
		invoke("projects:list-stateless") as Promise<Array<ProjectForFrontend>>,
	listReviews: (params) => invoke("workspace:list-reviews", params) as Promise<Array<ForgeReview>>,
	listReviewsForBranch: (params) =>
		invoke("workspace:list-reviews-for-branch", params) as Promise<Array<ForgeReview>>,
	mergeReview: (params) => invoke("workspace:merge-review", params) as Promise<void>,
	moveBranch: (params) => invoke("workspace:move-branch", params) as Promise<MoveBranchResult>,
	openInEditor: (params) => invoke("workspace:open-in-editor", params) as Promise<void>,
	openInWebBrowser: (url) => invoke("workspace:open-in-web-browser", url) as Promise<void>,
	pathJoin: (path, ...paths) => invoke("lite:path-join", path, ...paths) as Promise<string>,
	publishReview: (params) => invoke("workspace:publish-review", params) as Promise<ForgeReview>,
	updateBranchName: (params) =>
		invoke("workspace:update-branch-name", params) as Promise<UpdateBranchNameResult>,
	updateReview: (params) => invoke("workspace:update-review", params) as Promise<void>,
	tearOffBranch: (params) =>
		invoke("workspace:tear-off-branch", params) as Promise<MoveBranchResult>,
	peelRestoreSnapshot: (params) =>
		invoke("workspace:peel-restore-snapshot", params) as Promise<Snapshot | null>,
	workspaceBranchAndAncestorsPush: (params) =>
		invoke("workspace:push-stack", params) as Promise<PushResult>,
	removeBranch: (params) => invoke("workspace:remove-branch", params) as Promise<void>,
	restoreSnapshotWithKind: (params) =>
		invoke("workspace:restore-snapshot-with-kind", params) as Promise<void>,
	reviewTemplate: (projectId) =>
		invoke("workspace:review-template", projectId) as Promise<ReviewTemplateInfo | null>,
	setReviewAutoMerge: (params) =>
		invoke("workspace:set-review-auto-merge", params) as Promise<void>,
	setReviewDraftiness: (params) =>
		invoke("workspace:set-review-draftiness", params) as Promise<void>,
	setReviewTemplate: (params) => invoke("workspace:set-review-template", params) as Promise<void>,
	setTargetRefAndInitProject: (params) =>
		invoke("workspace:set-target-ref-and-init-project", params) as Promise<void>,
	showNativeMenu: (params) => invoke("lite:show-native-menu", params) as Promise<string | null>,
	treeChangeDiffs: (params) =>
		invoke("workspace:tree-change-diffs", params) as Promise<UnifiedPatch | null>,
	unapplyStack: (params) => invoke("workspace:unapply-stack", params) as Promise<void>,
	workspaceIntegrateUpstream: (params) =>
		invoke("workspace:integrate-upstream", params) as Promise<WorkspaceIntegrateUpstreamOutcome>,
	updateReviewFooters: (params) =>
		invoke("workspace:update-review-footers", params) as Promise<void>,
	warmCiChecksCache: (projectId) =>
		invoke("workspace:warm-ci-checks-cache", projectId) as Promise<void>,
	/**
	 * Subscribe to a project.
	 *
	 * This sets up a listener to project updates from the Rust-end.
	 *
	 * **Usage**
	 * It's expected that one window has max one subscription per project, although it is possible to have multiple.
	 * The node-end of the application will deduplicate project watchers (there will only ever be one watcher) but
	 * there is no deduplication in terms of project subscriptions.
	 *
	 * The responsability of subscribing once and correctly unsubscribing to a project is shifted to the UI.
	 *
	 * @param projectId - The ID of the project to subscribe to.
	 * @param callback - The callback function to pass the event information to.
	 * @returns A subscription ID.
	 */
	watcherSubscribe: async (projectId, callback) => {
		const { subscriptionId, eventChannel } = (await invoke("workspace:watcher-subscribe", {
			projectId,
		})) as WatcherSubscribeResult;
		startWatcherTraceStream();
		const listener = (_event: Electron.IpcRendererEvent, payload: WatcherEvent) => {
			traceWatcherEvent(projectId, subscriptionId, payload);
			callback(payload);
		};
		watcherListenerBySubscription.set(subscriptionId, { eventChannel, listener });
		ipcRenderer.on(eventChannel, listener);
		return subscriptionId;
	},
	/**
	 * Stop watching a project.
	 *
	 * Remove the listener for a given subscription channel.
	 * If this is the last subscription to a project, the watcher will stop.
	 * @param subscriptionId
	 */
	watcherUnsubscribe: async (subscriptionId) => {
		const registration = watcherListenerBySubscription.get(subscriptionId);
		if (registration) {
			ipcRenderer.removeListener(registration.eventChannel, registration.listener);
			watcherListenerBySubscription.delete(subscriptionId);
			if (watcherListenerBySubscription.size === 0) stopWatcherTraceStream();
		}
		return invoke("workspace:watcher-unsubscribe", {
			subscriptionId,
		}) as Promise<boolean>;
	},
	/**
	 * Stop all watchers.
	 *
	 * Remove all subscription listners and stop all watchers.
	 */
	watcherStopAll: async () => {
		for (const { eventChannel, listener } of watcherListenerBySubscription.values())
			ipcRenderer.removeListener(eventChannel, listener);

		watcherListenerBySubscription.clear();
		stopWatcherTraceStream();
		return invoke("workspace:watcher-stop-all") as Promise<number>;
	},
	readGUISettings: () => invoke("lite:gui-settings:read") as Promise<GUISettings>,
	writeGUISettings: (settings: GUISettings) =>
		invoke("lite:gui-settings:write", settings) as Promise<void>,
	platform: process.platform,
};

contextBridge.exposeInMainWorld("lite", api);
