import {
	getReviewMergeStatusQueryOptions,
	listCIChecksQueryOptions,
	treeChangesDiffsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { handleProjectEvent } from "#ui/project-events.ts";
import { invalidateTags } from "#ui/api/tags.ts";
import type {
	CiCheck,
	ReviewMergeStatus,
	TreeChange,
	TreeStatus,
	WatcherEvent,
} from "@gitbutler/but-sdk";
import { QueryClient, QueryObserver } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";

// The settle burst leaves timers pending, and `window` is stubbed per test;
// neither may outlive its test.
afterEach(() => {
	vi.clearAllTimers();
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

const check = (status: CiCheck["status"]): CiCheck =>
	({ name: "ci", status }) as unknown as CiCheck;
const running = check("inProgress");
const passed = check({ complete: { conclusion: "success", completed_at: null } });

/** Runs the checks query once per response, on the same client, and lists what it refetched. */
const pollChecks = async (responses: Array<Array<CiCheck> | Error>) => {
	vi.useFakeTimers();
	const client = new QueryClient();
	const refetched = vi.spyOn(client, "refetchQueries").mockResolvedValue();
	const listCiChecks = vi.fn();
	vi.stubGlobal("window", { lite: { listCiChecks } });
	const options = listCIChecksQueryOptions({
		projectId: "p1",
		reference: "feature",
		polling: "priority",
	});
	for (const response of responses) {
		if (response instanceof Error) listCiChecks.mockRejectedValueOnce(response);
		else listCiChecks.mockResolvedValueOnce(response);
		await client.fetchQuery({ ...options, staleTime: 0 });
	}
	// The merge-status burst starts on a timer.
	await vi.advanceTimersByTimeAsync(0);
	return refetched.mock.calls.map(([filters]) => filters?.queryKey);
};

describe("listCIChecksQueryOptions", () => {
	it("refetches the merge status once the checks reach a verdict", async () => {
		expect(await pollChecks([[running], [running], [passed]])).toEqual([
			["p1", "getReviewMergeStatus"],
		]);
	});

	it("leaves the merge status alone while checks are still running or already settled", async () => {
		expect(await pollChecks([[running], [running]])).toEqual([]);
		expect(await pollChecks([[passed], [passed]])).toEqual([]);
		expect(await pollChecks([[passed], [running]])).toEqual([]);
		// A failed listing is not a verdict.
		expect(await pollChecks([[running], new Error("422")])).toEqual([]);
	});
});

describe("getReviewMergeStatusQueryOptions", () => {
	it("polls briskly only while the forge is still computing", () => {
		const { refetchInterval } = getReviewMergeStatusQueryOptions({ projectId: "p1", reviewId: 1 });
		if (typeof refetchInterval !== "function") throw new Error("refetchInterval is a constant");
		const intervalFor = (mergeableState: string | null) =>
			refetchInterval({
				state: { data: { mergeableState, commentsCount: 0, isMergeable: false } },
			} as unknown as Parameters<typeof refetchInterval>[0]);
		expect(intervalFor("unknown")).toBe(10_000);
		expect(intervalFor("checking")).toBe(10_000);
		expect(intervalFor(null)).toBe(10_000);
		expect(intervalFor("blocked")).toBe(60_000);
		expect(intervalFor("clean")).toBe(60_000);
	});
});

describe("settling the merge status", () => {
	it("keeps asking on a backoff until the forge reports it mergeable", async () => {
		vi.useFakeTimers();
		const client = new QueryClient();
		const blocked: ReviewMergeStatus = {
			mergeableState: "blocked",
			commentsCount: 0,
			isMergeable: false,
		};
		const clean: ReviewMergeStatus = { ...blocked, mergeableState: "clean", isMergeable: true };
		const getReviewMergeStatus = vi
			.fn()
			.mockResolvedValueOnce(blocked)
			.mockResolvedValueOnce(blocked)
			.mockResolvedValueOnce(blocked)
			.mockResolvedValue(clean);
		const listCiChecks = vi
			.fn()
			.mockResolvedValueOnce([running])
			.mockResolvedValueOnce([passed])
			.mockResolvedValueOnce([running])
			.mockResolvedValueOnce([passed]);
		vi.stubGlobal("window", { lite: { listCiChecks, getReviewMergeStatus } });

		// An observer keeps the merge-status query active, as the PR page does.
		const observer = new QueryObserver(
			client,
			getReviewMergeStatusQueryOptions({ projectId: "p1", reviewId: 1 }),
		);
		const unsubscribe = observer.subscribe(() => {});
		await vi.advanceTimersByTimeAsync(0);
		expect(getReviewMergeStatus).toHaveBeenCalledTimes(1);

		const checks = listCIChecksQueryOptions({
			projectId: "p1",
			reference: "feature",
			polling: "priority",
		});
		await client.fetchQuery({ ...checks, staleTime: 0 });
		await client.fetchQuery({ ...checks, staleTime: 0 });
		// A second branch settling in the same window joins the burst under way.
		const other = listCIChecksQueryOptions({
			projectId: "p1",
			reference: "other",
			polling: "priority",
		});
		await client.fetchQuery({ ...other, staleTime: 0 });
		await client.fetchQuery({ ...other, staleTime: 0 });

		await vi.advanceTimersByTimeAsync(0);
		expect(getReviewMergeStatus).toHaveBeenCalledTimes(2);
		await vi.advanceTimersByTimeAsync(3_000);
		expect(getReviewMergeStatus).toHaveBeenCalledTimes(3);
		await vi.advanceTimersByTimeAsync(8_000);
		expect(getReviewMergeStatus).toHaveBeenCalledTimes(4);
		// Mergeable now, so the last look never happens.
		await vi.advanceTimersByTimeAsync(20_000);
		expect(getReviewMergeStatus).toHaveBeenCalledTimes(4);

		unsubscribe();
	});
});

describe("treeChangesDiffsQueryOptions", () => {
	it("reuses a hash only within the scope it was computed for", () => {
		// One array under two scopes, as the shared empty array of every clean worktree is.
		const changes: Array<TreeChange> = [];
		const main = treeChangesDiffsQueryOptions({ projectId: "p1", changes }).queryHash;
		const worktree = treeChangesDiffsQueryOptions({ projectId: "p1", changes, worktree: "wt" });
		const elsewhere = treeChangesDiffsQueryOptions({ projectId: "p2", changes });
		expect(worktree.queryHash).not.toBe(main);
		expect(elsewhere.queryHash).not.toBe(main);
		expect(treeChangesDiffsQueryOptions({ projectId: "p1", changes }).queryHash).toBe(main);
	});
});

describe("diffs after file events", () => {
	const change = (id: string): TreeChange => ({
		path: "file.txt",
		pathBytes: [102, 105, 108, 101, 46, 116, 120, 116],
		status: {
			type: "Modification",
			subject: {
				previousState: { id: "a".repeat(40), kind: "Blob" },
				state: { id, kind: "Blob" },
				flags: null,
			},
		},
	});
	const fileEvent = {
		name: "project://p1/worktree_changes",
		payload: {
			type: "worktreeChanges",
			subject: {
				changedPaths: ["file.txt"],
				changes: {
					changes: [],
					ignoredChanges: [],
					modificationTimes: {},
					assignments: [],
					assignmentsError: null,
					dependencies: null,
					dependenciesError: null,
				},
			},
		},
	} satisfies WatcherEvent;

	const state = { id: "b".repeat(40), kind: "Blob" } as const;
	const previousState = { ...state, id: "a".repeat(40) };
	it.each<TreeStatus>([
		{ type: "Addition", subject: { state, isUntracked: false } },
		{ type: "Deletion", subject: { previousState } },
		{ type: "Modification", subject: { state, previousState, flags: null } },
		{
			type: "Rename",
			subject: {
				state,
				previousState,
				previousPath: "old.txt",
				previousPathBytes: [],
				flags: null,
			},
		},
	])(
		"leaves blob $type diffs alone during file churn, but still honors other invalidations",
		async (status) => {
			const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity } } });
			const treeChangeDiffs = vi.fn().mockResolvedValue(null);
			vi.stubGlobal("window", { lite: { treeChangeDiffs } });
			vi.stubGlobal("navigator", { hardwareConcurrency: 2 });
			const options = treeChangesDiffsQueryOptions({
				projectId: "p1",
				changes: [{ ...change(state.id), status }],
			});
			await client.fetchQuery(options);
			const observer = new QueryObserver(client, options);
			const unsubscribe = observer.subscribe(() => {});
			try {
				for (let i = 0; i < 3; i++) {
					handleProjectEvent(fileEvent, "p1", client);
					await vi.waitFor(() => expect(client.isFetching()).toBe(0));
				}
				expect(treeChangeDiffs).toHaveBeenCalledTimes(1);
				await invalidateTags(client, ["Diffs"], "p1");
				expect(treeChangeDiffs).toHaveBeenCalledTimes(2);
				for (const payload of [
					{ type: "gitActivity", subject: { headSha: "head" } },
					{ type: "workspaceActivity", subject: null },
				] as const) {
					handleProjectEvent({ name: payload.type, payload }, "p1", client);
					await vi.waitFor(() => expect(client.isFetching()).toBe(0));
				}
				expect(treeChangeDiffs).toHaveBeenCalledTimes(4);
				for (const changedPaths of [[], undefined]) {
					handleProjectEvent(
						{
							...fileEvent,
							payload: {
								...fileEvent.payload,
								subject: { ...fileEvent.payload.subject, changedPaths },
							},
						} as WatcherEvent,
						"p1",
						client,
					);
					await vi.waitFor(() => expect(client.isFetching()).toBe(0));
				}
				expect(treeChangeDiffs).toHaveBeenCalledTimes(6);
			} finally {
				unsubscribe();
				client.clear();
			}
		},
	);

	it.each([undefined, "linked"])(
		"refreshes mixed live and blob diffs with unchanged descriptors in %s",
		async (worktree) => {
			const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity } } });
			const diff = vi.fn().mockResolvedValue(null);
			vi.stubGlobal("window", { lite: { treeChangeDiffs: diff, treeChangeDiffsFromSource: diff } });
			vi.stubGlobal("navigator", { hardwareConcurrency: 2 });
			const options = treeChangesDiffsQueryOptions({
				projectId: "p1",
				changes: [change(state.id), change("0".repeat(40))],
				worktree,
			});
			await client.fetchQuery(options);
			const observer = new QueryObserver(client, options);
			const unsubscribe = observer.subscribe(() => {});
			diff.mockResolvedValue({ type: "Binary" });
			try {
				handleProjectEvent(fileEvent, "p1", client);
				await vi.waitFor(() =>
					expect(client.getQueryData(options.queryKey)).toEqual([
						{ type: "Binary" },
						{ type: "Binary" },
					]),
				);
				expect(diff).toHaveBeenCalledTimes(4);
			} finally {
				unsubscribe();
				client.clear();
			}
		},
	);

	it("does not restart a blob diff stream that has already published a batch", async () => {
		const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity } } });
		const pending = Promise.withResolvers<null>();
		const diff = vi.fn(({ change }: { change: TreeChange }) =>
			change.path === "64" ? pending.promise : Promise.resolve(null),
		);
		vi.stubGlobal("window", { lite: { treeChangeDiffs: diff } });
		vi.stubGlobal("navigator", { hardwareConcurrency: 2 });
		const options = treeChangesDiffsQueryOptions({
			projectId: "p1",
			changes: Array.from({ length: 65 }, (_, i) => ({ ...change(state.id), path: String(i) })),
		});
		const observer = new QueryObserver(client, options);
		const unsubscribe = observer.subscribe(() => {});
		try {
			await vi.waitFor(() => expect(client.getQueryData(options.queryKey)).toHaveLength(64));
			handleProjectEvent(fileEvent, "p1", client);
			pending.resolve(null);
			await vi.waitFor(() => expect(client.isFetching()).toBe(0));
			expect(client.getQueryData(options.queryKey)).toHaveLength(65);
			expect(diff).toHaveBeenCalledTimes(65);
		} finally {
			pending.resolve(null);
			unsubscribe();
			client.clear();
		}
	});

	it("filters single-file diffs and conservatively refreshes queries without metadata", async () => {
		const client = new QueryClient();
		const blob = treeChangeDiffsQueryOptions({ projectId: "p1", change: change(state.id) });
		const live = treeChangeDiffsQueryOptions({ projectId: "p1", change: change("0".repeat(40)) });
		const unknown = { queryKey: ["p1", "treeChangeDiffs", "unknown"] as const };
		vi.stubGlobal("window", { lite: { treeChangeDiffs: vi.fn().mockResolvedValue(null) } });
		await client.fetchQuery(blob);
		await client.fetchQuery(live);
		client.setQueryData(unknown.queryKey, null);
		try {
			handleProjectEvent(fileEvent, "p1", client);
			expect(client.getQueryState(blob.queryKey)?.isInvalidated).toBe(false);
			expect(client.getQueryState(live.queryKey)?.isInvalidated).toBe(true);
			expect(client.getQueryState(unknown.queryKey)?.isInvalidated).toBe(true);
		} finally {
			client.clear();
		}
	});
});
