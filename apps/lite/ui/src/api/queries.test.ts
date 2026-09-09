import {
	getReviewMergeStatusQueryOptions,
	listCIChecksQueryOptions,
	treeChangesDiffsQueryOptions,
} from "#ui/api/queries.ts";
import type { CiCheck, ReviewMergeStatus, TreeChange } from "@gitbutler/but-sdk";
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
