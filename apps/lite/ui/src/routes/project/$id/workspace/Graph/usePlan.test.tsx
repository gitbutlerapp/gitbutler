/** @vitest-environment jsdom */
import type { RefInfo, TargetCommit, TargetCommitPage } from "@gitbutler/but-sdk";
import { configureStore } from "@reduxjs/toolkit";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, createRef, useImperativeHandle } from "react";
import { assert } from "#ui/assert.ts";
import { createRoot, type Root } from "react-dom/client";
import { Provider } from "react-redux";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { projectSlice } from "#ui/projects/state.ts";
import { usePlan, type Graph } from "./usePlan.ts";
import { sectionAddresses } from "./layout.ts";
import { guiSettingsQueryOptions } from "#ui/api/queries.ts";

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const projectId = "history-test";
const commit = (id: string, inWorkspace = true): TargetCommit => ({
	commit: {
		id,
		message: id,
		authoredAt: 0,
		committedAt: 0,
		changeId: null,
		author: { name: "", email: "", gravatarUrl: "" },
	},
	inWorkspace,
	review: null,
});
const listing: TargetCommitPage = {
	commits: [commit("incoming", false), commit("base")],
	hasMore: false,
};
const page = (prefix: string): TargetCommitPage => ({
	commits: Array.from({ length: 25 }, (_, i) => commit(`${prefix}${i}`)),
	hasMore: true,
});
const targetCommits = vi.fn();
let client: QueryClient;
let store: ReturnType<typeof createStore>;
let root: Root;
let container: HTMLDivElement;
const graphRef = createRef<Graph>();
const graph = () => assert(graphRef.current);
const createStore = () => configureStore({ reducer: { project: projectSlice.reducer } });
const Probe = () => {
	const result = usePlan(projectId);
	useImperativeHandle(graphRef, () => result, [result]);
	return null;
};
const flush = async () => {
	await act(async () => {
		await vi.advanceTimersByTimeAsync(20);
	});
};
const toggleHistory = async () => {
	await act(async () => {
		store.dispatch(projectSlice.actions.toggleGraphHistory({ projectId }));
	});
	await flush();
};
const showMoreHistory = async () => {
	await act(async () => {
		await graph().showMoreHistory();
	});
	await flush();
};

beforeEach(async () => {
	vi.useFakeTimers();
	targetCommits
		.mockReset()
		.mockImplementation(({ from }: { from: string | null }) =>
			Promise.resolve(from === null ? listing : from === "base" ? page("older") : page("oldest")),
		);
	vi.stubGlobal("lite", {
		readGUISettings: () => Promise.resolve({ version: 1 }),
		headInfo: () =>
			Promise.resolve({
				stacks: [],
				worktrees: [],
				target: {
					remoteTrackingRef: { displayName: "main", remoteName: "origin", fullNameBytes: [] },
					isCurrent: false,
					commitsAhead: 1,
				},
			} as unknown as RefInfo),
		workspaceTargetCommits: targetCommits,
	});
	client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
	store = createStore();
	container = document.createElement("div");
	document.body.append(container);
	root = createRoot(container);
	await act(async () => {
		root.render(
			<Provider store={store}>
				<QueryClientProvider client={client}>
					<Probe />
				</QueryClientProvider>
			</Provider>,
		);
	});
	await flush();
});

afterEach(async () => {
	await act(async () => {
		root.unmount();
	});
	client.clear();
	container.remove();
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

it("fetches older commits only when History opens, even when the base listing reports its natural end", async () => {
	expect(targetCommits).toHaveBeenCalledTimes(1);
	expect(graph().plan.history).toEqual([]);
	await toggleHistory();
	expect(targetCommits).toHaveBeenLastCalledWith({ projectId, from: "base", limit: 25 });
	expect(graph().plan.history).toHaveLength(5);
	expect(graph().plan.history[0]?.commit.id).toBe("base");
	expect(graph().plan.incoming).toEqual([]);
	expect(graph().plan.header?.incoming).toBe(1);
	expect(graph().historyMore).toBe("idle");

	await showMoreHistory();
	expect(graph().plan.history).toHaveLength(25);
	expect(targetCommits).toHaveBeenCalledTimes(2);
	await showMoreHistory();
	expect(targetCommits).toHaveBeenLastCalledWith({ projectId, from: "older24", limit: 25 });
	expect(graph().plan.history).toHaveLength(45);
	expect(new Set(graph().plan.history.map((c) => c.commit.id)).size).toBe(45);

	await toggleHistory();
	await toggleHistory();
	expect(graph().plan.history).toHaveLength(5);
	expect(targetCommits).toHaveBeenCalledTimes(3);
});

it("keeps loaded History on a failed page and lets the next ask retry the same cursor", async () => {
	await toggleHistory();
	await showMoreHistory();
	targetCommits.mockRejectedValueOnce(new Error("history unavailable"));
	await showMoreHistory();
	expect(graph().historyMore).toBe("failed");
	expect(graph().plan.history).toHaveLength(25);
	await showMoreHistory();
	expect(targetCommits).toHaveBeenLastCalledWith({ projectId, from: "older24", limit: 25 });
	expect(graph().historyMore).toBe("idle");
	expect(graph().plan.history).toHaveLength(45);
});

it("keeps History reset when a pending page finishes after it is collapsed", async () => {
	await toggleHistory();
	await showMoreHistory();
	const nextPage = Promise.withResolvers<TargetCommitPage>();
	targetCommits.mockReturnValueOnce(nextPage.promise);
	let request: Promise<void>;
	await act(async () => {
		request = graph().showMoreHistory();
	});
	await toggleHistory();
	await act(async () => {
		nextPage.resolve(page("oldest"));
		await request;
	});
	await flush();
	await toggleHistory();
	expect(graph().plan.history).toHaveLength(5);
});

it("discovers shared history after multiple clipped incoming pages", async () => {
	await act(async () => {
		client.setQueryData([projectId, "workspaceTargetCommits"], {
			commits: Array.from({ length: 1000 }, (_, index) => commit(`incoming${index}`, false)),
			hasMore: true,
		});
	});
	targetCommits.mockResolvedValueOnce({ commits: [commit("continued", false)], hasMore: true });
	expect(graph().plan.historyAvailable).toBe(true);
	await toggleHistory();
	expect(targetCommits).toHaveBeenLastCalledWith({ projectId, from: "incoming999", limit: 25 });
	expect(graph().plan.history).toEqual([]);
	expect(graph().historyMore).toBe("idle");
	targetCommits.mockResolvedValueOnce({
		commits: [commit("shared"), commit("older")],
		hasMore: false,
	});
	await showMoreHistory();
	expect(targetCommits).toHaveBeenLastCalledWith({ projectId, from: "continued", limit: 25 });
	expect(graph().plan.history.map((entry) => entry.commit.id)).toEqual(["shared", "older"]);
	expect(graph().plan.header?.incoming).toBe(1001);
	expect(graph().historyMore).toBe("hidden");
});

it("retries an initial History failure even with more than a page already cached", async () => {
	await act(async () => {
		client.setQueryData([projectId, "workspaceTargetCommits"], {
			commits: Array.from({ length: 30 }, (_, index) => commit(`shared${index}`)),
			hasMore: false,
		});
	});
	targetCommits.mockRejectedValueOnce(new Error("history unavailable"));
	await toggleHistory();
	expect(graph().historyMore).toBe("failed");
	expect(graph().plan.historyHidden).toBe(25);
	const before = targetCommits.mock.calls.length;
	await showMoreHistory();
	expect(targetCommits).toHaveBeenCalledTimes(before + 1);
	expect(targetCommits).toHaveBeenLastCalledWith({ projectId, from: "shared29", limit: 25 });
	expect(graph().historyMore).toBe("idle");
});

it("does not fetch or navigate hidden history after the target loses its shared tail", async () => {
	await toggleHistory();
	const before = targetCommits.mock.calls.length;
	await act(async () => {
		client.setQueryData([projectId, "workspaceTargetCommits"], {
			commits: [commit("unrelated", false)],
			hasMore: false,
		});
	});
	await flush();
	expect(targetCommits).toHaveBeenCalledTimes(before);
	expect(graph().plan.historyAvailable).toBe(false);
	expect(graph().plan.history).toEqual([]);
	expect(sectionAddresses(graph().plan)).toEqual([]);
	expect(graph().historyMore).toBe("hidden");
});

it("ignores cached pages for the same cursor once it is no longer shared", async () => {
	await toggleHistory();
	await act(async () => {
		client.setQueryData([projectId, "workspaceTargetCommits"], {
			commits: [commit("base", false)],
			hasMore: false,
		});
	});
	await flush();
	expect(graph().plan.history).toEqual([]);
	expect(sectionAddresses(graph().plan)).toEqual([]);
	expect(graph().historyMore).toBe("hidden");
});

it("stops offering History when a clipped walk ends without any shared commits", async () => {
	await act(async () => {
		client.setQueryData([projectId, "workspaceTargetCommits"], {
			commits: [commit("unrelated", false)],
			hasMore: true,
		});
	});
	targetCommits.mockResolvedValueOnce({ commits: [commit("root", false)], hasMore: false });
	await toggleHistory();
	expect(graph().plan.historyAvailable).toBe(false);
	expect(graph().plan.history).toEqual([]);
	expect(graph().historyMore).toBe("hidden");
	const before = targetCommits.mock.calls.length;
	await showMoreHistory();
	expect(targetCommits).toHaveBeenCalledTimes(before);
});

it("preserves stack placement identities while History opens and pages", async () => {
	const stacks = graph().stacks;
	const worktrees = graph().plan.worktrees;
	await toggleHistory();
	expect(graph().stacks).toBe(stacks);
	expect(graph().plan.worktrees).toBe(worktrees);
	await showMoreHistory();
	expect(graph().stacks).toBe(stacks);
	expect(graph().plan.worktrees).toBe(worktrees);
});

const setRecentHistory = async (recent = true) => {
	await act(async () => {
		client.setQueryData(guiSettingsQueryOptions.queryKey, {
			version: 1,
			historyDisplayMode: recent ? "last-12-hours" : "commits",
		});
	});
	await flush();
};
const timedCommit = (id: string, age: number, inWorkspace = true): TargetCommit => ({
	...commit(id, inWorkspace),
	commit: { ...commit(id).commit, committedAt: Date.now() - age },
});
const hour = 60 * 60 * 1000;

it("automatically loads the last 12 hours across pages and still reveals older commits on demand", async () => {
	const first = Array.from({ length: 25 }, (_, i) => timedCommit(`recent${i}`, hour));
	const second = [timedCommit("last-recent", 11 * hour), timedCommit("old", 13 * hour)];
	targetCommits.mockResolvedValueOnce({ commits: first, hasMore: true });
	targetCommits.mockResolvedValueOnce({ commits: second, hasMore: true });
	await setRecentHistory();
	expect(targetCommits).toHaveBeenCalledTimes(1);
	const stacks = graph().stacks;
	const worktrees = graph().plan.worktrees;
	await toggleHistory();
	expect(targetCommits).toHaveBeenCalledTimes(3);
	expect(targetCommits).toHaveBeenLastCalledWith({ projectId, from: "recent24", limit: 25 });
	expect(graph().plan.history.at(-1)?.commit.id).toBe("last-recent");
	expect(graph().plan.history).toHaveLength(27);
	expect(graph().plan.historyHidden).toBe(1);
	expect(graph().stacks).toBe(stacks);
	expect(graph().plan.worktrees).toBe(worktrees);
	targetCommits.mockResolvedValueOnce({ commits: [commit("oldest")], hasMore: false });
	await showMoreHistory();
	expect(graph().plan.history.at(-1)?.commit.id).toBe("oldest");
	expect(graph().historyMore).toBe("hidden");
	await toggleHistory();
	await toggleHistory();
	expect(graph().plan.history.at(-1)?.commit.id).toBe("last-recent");
});

it("lets Show more reveal history when there are no commits in the last 12 hours", async () => {
	await setRecentHistory();
	await toggleHistory();
	expect(graph().plan.history).toEqual([]);
	expect(graph().historyMore).toBe("idle");
	await showMoreHistory();
	expect(graph().plan.history).toHaveLength(20);
	expect(graph().plan.history[0]?.commit.id).toBe("base");
});

it("keeps walking old incoming pages until it reaches shared history", async () => {
	await act(async () => {
		client.setQueryData([projectId, "workspaceTargetCommits"], {
			commits: [commit("incoming", false)],
			hasMore: true,
		});
	});
	targetCommits.mockResolvedValueOnce({ commits: [commit("continued", false)], hasMore: true });
	targetCommits.mockResolvedValueOnce({
		commits: [timedCommit("shared", hour), commit("old")],
		hasMore: false,
	});
	await setRecentHistory();
	await toggleHistory();
	expect(graph().plan.history.map((entry) => entry.commit.id)).toEqual(["shared"]);
	expect(graph().plan.header?.incoming).toBe(2);
});

it("updates the time window while open without refetching its cached history", async () => {
	targetCommits.mockResolvedValueOnce({
		commits: [timedCommit("recent", 12 * hour - 30_000), commit("old")],
		hasMore: false,
	});
	await setRecentHistory();
	await toggleHistory();
	expect(graph().plan.history.at(-1)?.commit.id).toBe("recent");
	await act(async () => {
		await vi.advanceTimersByTimeAsync(60_000);
	});
	expect(graph().plan.history).toEqual([]);
	expect(targetCommits).toHaveBeenCalledTimes(2);
	await setRecentHistory(false);
	expect(graph().plan.history).toHaveLength(5);
});

it("does not expand a newly selected mode when an earlier Show more request finishes", async () => {
	await toggleHistory();
	await showMoreHistory();
	const next = Promise.withResolvers<TargetCommitPage>();
	targetCommits.mockReturnValueOnce(next.promise);
	let request: Promise<void>;
	await act(async () => {
		request = graph().showMoreHistory();
	});
	await act(async () => {
		store.dispatch(projectSlice.actions.resetGraphHistory({ projectId }));
	});
	await setRecentHistory();
	await act(async () => {
		next.resolve(page("oldest"));
		await request;
	});
	await flush();
	expect(graph().plan.history).toEqual([]);
	expect(projectSlice.selectors.selectGraphFolds(store.getState(), projectId).moreHistory).toBe(0);
});
