import ProjectLayout from "./+layout.svelte";
import { BACKEND } from "$lib/backend";
import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
import { BRANCH_SERVICE } from "$lib/branches/branchService.svelte";
import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
import { GITLAB_USER_SERVICE } from "$lib/forge/gitlab/gitlabUserService.svelte";
import { LISTING_SERVICE, ListingService } from "$lib/forge/listingService.svelte";
import { GIT_SERVICE } from "$lib/git/gitService";
import { MODE_SERVICE } from "$lib/mode/modeService";
import { PROJECTS_SERVICE } from "$lib/project/projectsService";
import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
import { UNCOMMITTED_SERVICE } from "$lib/selection/uncommittedService.svelte";
import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
import { butlerModule } from "$lib/state/butlerModule";
import { CLIENT_STATE } from "$lib/state/clientState.svelte";
import { ReduxTag } from "$lib/state/tags";
import { POSTHOG_WRAPPER } from "$lib/telemetry/posthog";
import { WORKTREE_SERVICE } from "$lib/worktree/worktreeService.svelte";
import { configureStore } from "@reduxjs/toolkit";
import { buildCreateApi, coreModule, QueryStatus } from "@reduxjs/toolkit/query";
import { render } from "@testing-library/svelte";
import { flushSync } from "svelte";
import { writable } from "svelte/store";
import { afterEach, describe, expect, test, vi } from "vitest";
import type { HookContext } from "$lib/state/context";

vi.mock("$app/navigation", () => ({ goto: vi.fn() }));
vi.mock("$lib/error/showError", () => ({ showError: vi.fn() }));
vi.mock("$components/settings/ProjectSettingsShortcutHandler.svelte", () => ({
	default: () => {},
}));
vi.mock("$components/shared/AnalyticsMonitor.svelte", () => ({ default: () => {} }));
vi.mock("$components/shared/FullviewLoading.svelte", () => ({ default: () => {} }));
vi.mock("$components/shared/NotOnGitButlerBranch.svelte", () => ({ default: () => {} }));
vi.mock("$components/shared/ProjectShortcutHandler.svelte", () => ({ default: () => {} }));
vi.mock("$components/shared/ReduxResult.svelte", () => ({ default: () => {} }));
vi.mock("$components/views/AppLayout.svelte", () => ({ default: () => {} }));
vi.mock("$components/views/NoBaseBranch.svelte", () => ({ default: () => {} }));
vi.mock("$components/views/ProblemLoadingRepo.svelte", () => ({ default: () => {} }));

const PROJECT_ID = "project";
const POLL_INTERVAL = 15 * 60 * 1000;

const review = {
	htmlUrl: "https://example.invalid/review/1",
	number: 1,
	title: "Cached review",
	body: null,
	author: null,
	labels: [],
	draft: false,
	sourceBranch: "topic",
	targetBranch: "main",
	sha: "deadbeef",
	createdAt: null,
	modifiedAt: null,
	mergedAt: null,
	closedAt: null,
	repositorySshUrl: null,
	repositoryHttpsUrl: null,
	repoOwner: null,
	headRepoIsFork: false,
	reviewers: [],
};

const success = { data: [review] };
const terminalError = {
	error: {
		origin: "ipc" as const,
		name: "API error: (list_reviews)",
		message: "The repository is unavailable to this account.",
		code: "GitHubInsufficientPermissions" as const,
	},
};
const nonterminalError = {
	error: {
		origin: "ipc" as const,
		name: "API error: (list_reviews)",
		message: "Temporary forge failure.",
		code: "Unknown" as const,
	},
};

class ReactiveStoreState {
	root = $state.raw<any>();
	readonly unsubscribe: () => void;

	constructor(private readonly store: ReturnType<typeof configureStore>) {
		this.root = store.getState();
		this.unsubscribe = store.subscribe(() => (this.root = store.getState()));
	}
}

function query(response?: unknown) {
	return {
		response,
		result: {
			status: response === undefined ? QueryStatus.uninitialized : QueryStatus.fulfilled,
			data: response,
		},
	};
}

type Response = typeof success | typeof terminalError | typeof nonterminalError;

function setup(
	responses: Array<Response | Promise<Response>>,
	listingServiceOverride?: Pick<ListingService, "list">,
) {
	let calls = 0;
	const storeRef = {} as {
		state: ReactiveStoreState;
		store: ReturnType<typeof configureStore>;
	};
	const context: HookContext = {
		getState: () => storeRef.state.root,
		getDispatch: () => storeRef.store.dispatch,
	};
	const api = buildCreateApi(
		coreModule(),
		butlerModule(context),
	)({
		reducerPath: "backend",
		tagTypes: Object.values(ReduxTag),
		baseQuery: async () => await responses[Math.min(calls++, responses.length - 1)]!,
		endpoints: () => ({}),
	});
	const store = configureStore({
		reducer: { [api.reducerPath]: api.reducer },
		middleware: (defaults) => defaults().concat(api.middleware),
	});
	const storeState = new ReactiveStoreState(store);
	storeRef.store = store;
	storeRef.state = storeState;
	const listingService =
		listingServiceOverride ?? new ListingService(api as never, store.dispatch as never);
	const forgeInfo = { capabilities: { listService: true } };
	function noop() {}
	async function asyncNoop() {}
	const contextMap = new Map<any, any>([
		[
			BACKEND._key,
			{
				getAppInfo: async () => ({ name: "GitButler" }),
				getWindowTitle: async () => "GitButler",
				setWindowTitle: noop,
				listen: () => noop,
			},
		],
		[
			BASE_BRANCH_SERVICE._key,
			{
				repo: () => query(),
				baseBranch: () => query(),
				refreshBaseBranch: asyncNoop,
				fetchFromRemotes: asyncNoop,
			},
		],
		[BRANCH_SERVICE._key, { refresh: asyncNoop }],
		[FORGE_INFO_SERVICE._key, { get: () => query(forgeInfo) }],
		[GITLAB_USER_SERVICE._key, { migrate: noop }],
		[LISTING_SERVICE._key, listingService],
		[GIT_SERVICE._key, { onFetch: () => noop }],
		[MODE_SERVICE._key, { mode: () => query(), head: () => query() }],
		[POSTHOG_WRAPPER._key, { setPostHogRepo: noop, captureOnboarding: noop }],
		[PROJECTS_SERVICE._key, { projects: () => query([]), setActiveProject: async () => undefined }],
		[FILE_SELECTION_MANAGER._key, { retain: noop }],
		[UNCOMMITTED_SERVICE._key, { updateData: noop }],
		[SETTINGS_SERVICE._key, { appSettings: writable({ fetch: { autoFetchIntervalMinutes: -1 } }) }],
		[STACK_SERVICE._key, { invalidateStacksAndDetails: noop }],
		[
			CLIENT_STATE._key,
			{
				dispatch: store.dispatch,
				backendApi: { util: { resetApiState: () => ({ type: "test/noop" }) } },
			},
		],
		[WORKTREE_SERVICE._key, { worktreeData: () => query() }],
	]);
	const rendered = render(ProjectLayout, {
		props: { data: { projectId: PROJECT_ID } } as never,
		context: contextMap,
	});

	return {
		api,
		store,
		storeState,
		rendered,
		get calls() {
			return calls;
		},
	};
}

async function settle() {
	flushSync();
	await vi.advanceTimersByTimeAsync(0);
	flushSync();
}

afterEach(() => {
	vi.useRealTimers();
	vi.restoreAllMocks();
});

describe("project review-list polling", () => {
	test("keeps cached reviews, stops terminal polling, and recovers through explicit retries", async () => {
		vi.useFakeTimers();
		const harness = setup([success, terminalError, terminalError, success, success]);
		await settle();
		expect(harness.calls).toBe(1);

		await vi.advanceTimersByTimeAsync(POLL_INTERVAL);
		const failed = (harness.api.endpoints as any).listPrs.select(PROJECT_ID)(
			harness.store.getState(),
		);
		expect(failed.data.ids).toEqual(["topic"]);
		expect(failed.isError).toBe(true);

		await vi.advanceTimersByTimeAsync(POLL_INTERVAL);
		expect(harness.calls, "terminal failure scheduled another interval request").toBe(2);

		harness.store.dispatch(harness.api.internalActions.onFocusLost());
		harness.store.dispatch(harness.api.internalActions.onFocus());
		await settle();
		expect(harness.calls).toBe(3);
		await vi.advanceTimersByTimeAsync(POLL_INTERVAL);
		expect(harness.calls).toBe(3);

		harness.store.dispatch(harness.api.internalActions.onFocusLost());
		harness.store.dispatch(harness.api.internalActions.onFocus());
		await settle();
		expect(harness.calls).toBe(4);

		await vi.advanceTimersByTimeAsync(POLL_INTERVAL - 1);
		expect(harness.calls).toBe(4);
		await vi.advanceTimersByTimeAsync(1);
		expect(harness.calls).toBe(5);

		harness.rendered.unmount();
		harness.storeState.unsubscribe();
	});

	test("keeps terminal polling stopped after a nonterminal failed retry", async () => {
		vi.useFakeTimers();
		let failRetry!: (result: typeof nonterminalError) => void;
		const retry = new Promise<typeof nonterminalError>((resolve) => (failRetry = resolve));
		const harness = setup([success, terminalError, retry]);
		await settle();
		expect(harness.calls).toBe(1);
		await vi.advanceTimersByTimeAsync(POLL_INTERVAL);
		expect(harness.calls).toBe(2);
		await vi.advanceTimersByTimeAsync(POLL_INTERVAL);
		expect(harness.calls).toBe(2);

		harness.store.dispatch(harness.api.internalActions.onFocusLost());
		harness.store.dispatch(harness.api.internalActions.onFocus());
		await settle();
		expect(harness.calls).toBe(3);
		const pending = (harness.api.endpoints as any).listPrs.select(PROJECT_ID)(
			harness.store.getState(),
		);
		expect(pending.isLoading).toBe(true);
		expect(pending.isError).toBe(false);

		failRetry(nonterminalError);
		await settle();

		await vi.advanceTimersByTimeAsync(POLL_INTERVAL);
		expect(harness.calls, "failed focus retry restarted terminal polling").toBe(3);

		harness.rendered.unmount();
		harness.storeState.unsubscribe();
	});

	test("resets terminal polling when the project changes", async () => {
		vi.useFakeTimers();
		let result = $state.raw<any>({
			status: QueryStatus.rejected,
			error: terminalError.error,
			startedTimeStamp: 1,
		});
		const intervals: Array<[string, number | undefined]> = [];
		const listingService = {
			list(projectId: string, pollingInterval?: number) {
				intervals.push([projectId, pollingInterval]);
				return {
					get result() {
						return result;
					},
				} as never;
			},
		};
		const harness = setup([], listingService);
		await settle();
		expect(intervals.at(-1)).toEqual([PROJECT_ID, 0]);

		result = { status: QueryStatus.pending, startedTimeStamp: 2 };
		await harness.rendered.rerender({ data: { projectId: "project-b" } } as never);
		await settle();
		result = {
			status: QueryStatus.rejected,
			error: nonterminalError.error,
			startedTimeStamp: 2,
		};
		await settle();

		expect(intervals.at(-1), "new project inherited terminal polling state").toEqual([
			"project-b",
			POLL_INTERVAL,
		]);
		harness.rendered.unmount();
		harness.storeState.unsubscribe();
	});

	test("keeps the 15-minute cadence after a nonterminal failure", async () => {
		vi.useFakeTimers();
		const harness = setup([success, nonterminalError]);
		await settle();
		await vi.advanceTimersByTimeAsync(POLL_INTERVAL * 2);
		expect(harness.calls).toBe(3);
		harness.rendered.unmount();
		harness.storeState.unsubscribe();
	});
});
