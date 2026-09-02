import { describe, expect, test } from "vitest";
import {
	apiCommandFailureLimitConfig,
	createApiCommandSampler,
} from "../../electron/src/api-command-sampling.ts";

describe("api command sampling", () => {
	test.each([
		"treeChangeDiffs",
		"branchDiff",
		"changesInWorktree",
		"commentsList",
		"commitDetailsWithLineStats",
		"headInfo",
		"listProjectsStateless",
		"listReviews",
		"workspaceFetchStatus",
		"workspaceTargetCommits",
	])("samples high-volume background read %s at 1%", (command) => {
		expect(createApiCommandSampler({ random: () => 0.009 }).sample(command, false)).toEqual({
			samplingRate: 0.01,
		});
		expect(createApiCommandSampler({ random: () => 0.01 }).sample(command, false)).toBeNull();
	});

	test.each([
		"branchDetails",
		"branchList",
		"getReview",
		"getReviewMergeStatus",
		"listCiChecks",
		"listReviewComments",
		"listReviewReactions",
		"listReviewSubmissions",
		"listReviewThreads",
		"listReviewTimelineEvents",
	])("samples automatic read %s", (command) => {
		expect(createApiCommandSampler({ random: () => 0.09 }).sample(command, false)).toEqual({
			samplingRate: 0.1,
		});
		expect(createApiCommandSampler({ random: () => 0.1 }).sample(command, false)).toBeNull();
	});

	test("always captures user-triggered commands", () => {
		expect(createApiCommandSampler({ random: () => 0.99 }).sample("commitCreate", false)).toEqual({
			samplingRate: 1,
		});
	});
});

describe("api command failure limiting", () => {
	test("captures intermittent failures", () => {
		let now = 0;
		const { sample } = createApiCommandSampler({ now: () => now });

		for (let index = 0; index < 20; index++) {
			now = index * 60_000;
			expect(sample("treeChangeDiffs", true)).toEqual({
				occurrenceCount: 1,
				samplingRate: 1,
			});
		}
	});

	test("limits bursts and carries suppressed occurrences into the next event", () => {
		let now = 0;
		const { sample } = createApiCommandSampler({ now: () => now });

		expect(sample("treeChangeDiffs", true)?.occurrenceCount).toBe(1);
		expect(sample("treeChangeDiffs", true)).toBeNull();
		now = 59_999;
		expect(sample("treeChangeDiffs", true)).toBeNull();
		now = 60_000;
		expect(sample("treeChangeDiffs", true)?.occurrenceCount).toBe(3);
	});

	test("limits each command independently", () => {
		const { sample } = createApiCommandSampler({
			failureLimit: { bucketSize: 1, refillIntervalMs: 10_000 },
			now: () => 0,
		});

		expect(sample("treeChangeDiffs", true)?.occurrenceCount).toBe(1);
		expect(sample("treeChangeDiffs", true)).toBeNull();
		expect(sample("workspaceFetchStatus", true)?.occurrenceCount).toBe(1);
	});

	test("success sampling does not consume or reset failure tokens", () => {
		let now = 0;
		let random = 0;
		const { sample } = createApiCommandSampler({
			failureLimit: { bucketSize: 1, refillIntervalMs: 10_000 },
			now: () => now,
			random: () => random,
		});

		expect(sample("treeChangeDiffs", false)).not.toBeNull();
		expect(sample("treeChangeDiffs", true)?.occurrenceCount).toBe(1);
		expect(sample("treeChangeDiffs", true)).toBeNull();
		random = 0.99;
		expect(sample("treeChangeDiffs", false)).toBeNull();
		expect(sample("treeChangeDiffs", true)).toBeNull();
		now = 10_000;
		expect(sample("treeChangeDiffs", true)?.occurrenceCount).toBe(3);
	});

	test("caps refilled tokens after a long idle", () => {
		let now = 0;
		const { sample } = createApiCommandSampler({
			failureLimit: { bucketSize: 2, refillIntervalMs: 10_000 },
			now: () => now,
		});

		expect(sample("treeChangeDiffs", true)).not.toBeNull();
		expect(sample("treeChangeDiffs", true)).not.toBeNull();
		expect(sample("treeChangeDiffs", true)).toBeNull();
		now = 100_000;
		expect(sample("treeChangeDiffs", true)?.occurrenceCount).toBe(2);
		expect(sample("treeChangeDiffs", true)?.occurrenceCount).toBe(1);
		expect(sample("treeChangeDiffs", true)).toBeNull();
	});

	test("drains suppressed failure tails once", () => {
		const sampler = createApiCommandSampler({
			failureLimit: { bucketSize: 1, refillIntervalMs: 60_000 },
			now: () => 0,
		});

		const { sample } = sampler;
		expect(sample("treeChangeDiffs", true)?.occurrenceCount).toBe(1);
		expect(sample("treeChangeDiffs", true)).toBeNull();
		expect(sample("treeChangeDiffs", true)).toBeNull();
		expect(sampler.drainSuppressedFailures()).toEqual([
			{ command: "treeChangeDiffs", occurrenceCount: 2 },
		]);
		expect(sampler.drainSuppressedFailures()).toEqual([]);
	});

	test("accepts a validated feature flag payload", () => {
		expect(apiCommandFailureLimitConfig({ bucketSize: 20, refillIntervalSeconds: 5 })).toEqual({
			bucketSize: 20,
			refillIntervalMs: 5_000,
		});
		expect(apiCommandFailureLimitConfig({ bucketSize: 0, refillIntervalSeconds: "fast" })).toEqual({
			bucketSize: 1,
			refillIntervalMs: 60_000,
		});
		expect(apiCommandFailureLimitConfig({ bucketSize: 1_001, refillIntervalSeconds: 10 })).toEqual({
			bucketSize: 1,
			refillIntervalMs: 60_000,
		});
	});
});
