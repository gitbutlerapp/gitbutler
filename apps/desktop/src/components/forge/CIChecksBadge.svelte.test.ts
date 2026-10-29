import CIChecksBadge from "$components/forge/CIChecksBadge.svelte";
import { CHECKS_MONITOR, ChecksMonitor } from "$lib/forge/checksMonitor.svelte";
import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
import { injectBackendEndpoints as injectGitLabEndpoints } from "$lib/forge/gitlab/gitlabUserService.svelte";
import { butlerModule } from "$lib/state/butlerModule";
import { ReduxTag } from "$lib/state/tags";
import { UI_STATE } from "$lib/state/uiState.svelte";
import { configureStore } from "@reduxjs/toolkit";
import { buildCreateApi, coreModule, QueryStatus } from "@reduxjs/toolkit/query";
import { render } from "@testing-library/svelte";
import { flushSync } from "svelte";
import { afterEach, describe, expect, test, vi } from "vitest";
import type { HookContext } from "$lib/state/context";

const PROJECT_ID = "project";
const BRANCH = "topic";
// The initial progressive interval while checks are still running.
const POLL_INTERVAL = 5 * 1000;

// A check that is still running, so completion never stops the poller and
// only the error classification can.
const runningCheck = {
	data: [
		{
			id: 1,
			name: "build",
			output: { summary: "", text: "", title: "" },
			startedAt: null,
			status: "inProgress",
			headSha: "deadbeef",
			url: "",
			htmlUrl: "",
			detailsUrl: "",
			pullRequests: [],
			reference: BRANCH,
			lastSyncAt: "",
		},
	],
};
// What `list_ci_checks` returns when GitLab answers 401 on the pipeline read.
const rejectedGitLabTokenError = {
	error: {
		origin: "ipc" as const,
		name: "API error: (list_ci_checks)",
		message: "GitLab did not accept the token.",
		code: "GitLabUnauthorized" as const,
	},
};
const rejectedTokenStore = {
	error: {
		origin: "ipc" as const,
		name: "API error: (store_gitlab_selfhosted_pat)",
		message: "GitLab did not accept the token.",
		code: "GitLabUnauthorized" as const,
	},
};
const acceptedTokenStore = {
	data: { username: "alice", name: null, email: null, host: "https://gitlab.example" },
};

class ReactiveStoreState {
	root = $state.raw<any>();
	readonly unsubscribe: () => void;

	constructor(private readonly store: ReturnType<typeof configureStore>) {
		this.root = store.getState();
		this.unsubscribe = store.subscribe(() => (this.root = store.getState()));
	}
}

function setup(
	responses: Array<typeof runningCheck | typeof rejectedGitLabTokenError>,
	tokenStores: Array<typeof rejectedTokenStore | typeof acceptedTokenStore>,
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
		baseQuery: async (_args: unknown, _api: unknown, extra: any) => {
			if (extra?.command === "list_ci_checks") {
				return responses[Math.min(calls++, responses.length - 1)]!;
			}
			if (extra?.command === "store_gitlab_selfhosted_pat") {
				return tokenStores.shift() ?? { data: null };
			}
			return { data: null };
		},
		endpoints: () => ({}),
	});
	const store = configureStore({
		reducer: { [api.reducerPath]: api.reducer },
		middleware: (defaults) => defaults().concat(api.middleware),
	});
	const storeState = new ReactiveStoreState(store);
	storeRef.store = store;
	storeRef.state = storeState;
	const gitlab = injectGitLabEndpoints(api as never);

	// The branches the badge is allowed to poll, kept reactive so the
	// component's own stop/restart bookkeeping works as in the app.
	let branchesToPoll = $state.raw([BRANCH]);
	const uiState = {
		project: () => ({
			branchesToPoll: {
				get current() {
					return branchesToPoll;
				},
				add: (name: string) => (branchesToPoll = [...branchesToPoll, name]),
				remove: (name: string) => (branchesToPoll = branchesToPoll.filter((b) => b !== name)),
			},
		}),
	};
	const forgeInfo = { capabilities: { checks: true } };
	const contextMap = new Map<any, any>([
		[CHECKS_MONITOR._key, new ChecksMonitor(api as never)],
		[
			FORGE_INFO_SERVICE._key,
			{ get: () => ({ response: forgeInfo, result: { status: QueryStatus.fulfilled } }) },
		],
		[UI_STATE._key, uiState],
	]);
	const rendered = render(CIChecksBadge, {
		props: { projectId: PROJECT_ID, branchName: BRANCH },
		context: contextMap,
	});
	function badge() {
		return rendered.getByTestId("pr-checks-badge");
	}

	return {
		api,
		gitlab,
		store,
		storeState,
		rendered,
		badge,
		get calls() {
			return calls;
		},
	};
}

// A store update reaches the badge a short moment later, well inside one
// polling interval.
const SETTLE_MS = 100;

async function settle() {
	flushSync();
	await vi.advanceTimersByTimeAsync(SETTLE_MS);
	flushSync();
}

afterEach(() => {
	vi.useRealTimers();
	vi.restoreAllMocks();
});

describe("CIChecksBadge polling", () => {
	test("stops on a rejected GitLab token and resumes once a replacement is stored", async () => {
		vi.useFakeTimers();
		const harness = setup(
			[runningCheck, rejectedGitLabTokenError, runningCheck],
			[rejectedTokenStore, acceptedTokenStore],
		);
		function badgeText() {
			return harness.badge().querySelector("[data-pr-text]")?.textContent?.trim();
		}
		await settle();
		expect(harness.calls).toBe(1);
		expect(badgeText()).toBe("Running");

		await vi.advanceTimersByTimeAsync(POLL_INTERVAL);
		await settle();
		expect(harness.calls).toBe(2);
		expect(badgeText()).toBe("Error");
		await vi.advanceTimersByTimeAsync(POLL_INTERVAL * 100);
		await settle();
		expect(harness.calls, "a rejected token kept polling the checks").toBe(2);

		async function store() {
			await harness.store.dispatch(
				harness.gitlab.endpoints.storeGitLabEnterprisePat.initiate({
					host: "https://gitlab.example",
					accessToken: "synthetic",
				}),
			);
		}
		// A token GitLab also rejects invalidates nothing: the checks would
		// only fail the same way again.
		await store();
		await settle();
		expect(harness.calls, "a failed token store refetched the checks").toBe(2);
		expect(badgeText()).toBe("Error");

		await store();
		await settle();
		expect(harness.calls, "an accepted token did not refetch the checks").toBe(3);
		expect(badgeText()).toBe("Running");

		await vi.advanceTimersByTimeAsync(POLL_INTERVAL - 3 * SETTLE_MS);
		expect(harness.calls).toBe(3);
		await vi.advanceTimersByTimeAsync(6 * SETTLE_MS);
		await settle();
		expect(harness.calls, "polling did not resume after the token was replaced").toBe(4);

		harness.rendered.unmount();
		harness.storeState.unsubscribe();
	});
});
