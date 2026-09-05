import { createPollBackoff } from "$lib/forge/shared/pollErrorBackoff.svelte";
import { QueryStatus } from "@reduxjs/toolkit/query";
import { flushSync } from "svelte";
import { describe, expect, test } from "vitest";

const INITIAL = 5 * 1000;
const SHORT = 30 * 1000;
const MEDIUM = 5 * 60 * 1000;

const terminal = { code: "GitHubInsufficientPermissions", message: "no access" };
const transient = { code: "Unknown", message: "flaky" };

/** An unkeyed consumer, like the PR and checks cards, recording each interval change. */
function unkeyed() {
	let result = $state.raw<any>(undefined);
	const intervals: number[] = [];
	const dispose = $effect.root(() => {
		const backoff = createPollBackoff({
			getResult: () => result,
			getElapsedMs: () => 0,
			getShouldStop: () => false,
		});
		$effect(() => {
			const interval = backoff.pollingInterval;
			if (intervals.at(-1) !== interval) intervals.push(interval);
		});
	});
	flushSync();
	return {
		intervals,
		dispose,
		set(status: QueryStatus, startedTimeStamp: number, error?: unknown) {
			result = { status, startedTimeStamp, error };
			flushSync();
		},
	};
}

describe("createPollBackoff", () => {
	test("keeps escalating across in-flight polls that keep failing", () => {
		const backoff = unkeyed();
		backoff.set(QueryStatus.rejected, 1, transient);
		backoff.set(QueryStatus.pending, 2);
		backoff.set(QueryStatus.rejected, 2, transient);
		backoff.set(QueryStatus.pending, 3);
		backoff.set(QueryStatus.rejected, 3, transient);
		// Each pending poll would otherwise clear the count, flipping the interval
		// back and forth once per completion instead of settling at medium.
		expect(backoff.intervals).toEqual([INITIAL, SHORT, MEDIUM]);
		backoff.dispose();
	});

	test("stays stopped after a terminal failure until a success", () => {
		const backoff = unkeyed();
		backoff.set(QueryStatus.rejected, 1, terminal);
		backoff.set(QueryStatus.pending, 2);
		backoff.set(QueryStatus.rejected, 2, transient);
		expect(backoff.intervals).toEqual([INITIAL, 0]);
		backoff.set(QueryStatus.fulfilled, 3);
		expect(backoff.intervals.at(-1)).toBe(INITIAL);
		backoff.dispose();
	});
});
