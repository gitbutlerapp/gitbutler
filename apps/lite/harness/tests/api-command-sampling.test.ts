import { describe, expect, test } from "vitest";
import {
	apiCommandFailureLimitConfig,
	createApiCommandSampler,
} from "../../electron/src/api-command-sampling.ts";

describe("api command sampling", () => {
	test("heavily samples watcher-driven diff reads", () => {
		expect(createApiCommandSampler({ random: () => 0.009 })("treeChangeDiffs", false)).toEqual({
			samplingRate: 0.01,
		});
		expect(createApiCommandSampler({ random: () => 0.01 })("treeChangeDiffs", false)).toBeNull();
	});

	test("samples other high-volume background reads", () => {
		expect(createApiCommandSampler({ random: () => 0.09 })("workspaceFetchStatus", false)).toEqual({
			samplingRate: 0.1,
		});
		expect(
			createApiCommandSampler({ random: () => 0.1 })("workspaceFetchStatus", false),
		).toBeNull();
	});

	test("always captures user-triggered commands", () => {
		expect(createApiCommandSampler({ random: () => 0.99 })("commitCreate", false)).toEqual({
			samplingRate: 1,
		});
	});
});

describe("api command failure limiting", () => {
	test("captures intermittent failures", () => {
		let now = 0;
		const sample = createApiCommandSampler({ now: () => now });

		for (let index = 0; index < 20; index++) {
			now = index * 10_000;
			expect(sample("treeChangeDiffs", true)).toEqual({
				occurrenceCount: 1,
				samplingRate: 1,
			});
		}
	});

	test("limits bursts and carries suppressed occurrences into the next event", () => {
		let now = 0;
		const sample = createApiCommandSampler({ now: () => now });

		for (let index = 0; index < 10; index++)
			expect(sample("treeChangeDiffs", true)?.occurrenceCount).toBe(1);

		expect(sample("treeChangeDiffs", true)).toBeNull();
		now = 9_999;
		expect(sample("treeChangeDiffs", true)).toBeNull();
		now = 10_000;
		expect(sample("treeChangeDiffs", true)?.occurrenceCount).toBe(3);
	});

	test("limits each command independently", () => {
		const sample = createApiCommandSampler({
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
		const sample = createApiCommandSampler({
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
		const sample = createApiCommandSampler({
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

	test("accepts a validated feature flag payload", () => {
		expect(apiCommandFailureLimitConfig({ bucketSize: 20, refillIntervalSeconds: 5 })).toEqual({
			bucketSize: 20,
			refillIntervalMs: 5_000,
		});
		expect(apiCommandFailureLimitConfig({ bucketSize: 0, refillIntervalSeconds: "fast" })).toEqual({
			bucketSize: 10,
			refillIntervalMs: 10_000,
		});
		expect(apiCommandFailureLimitConfig({ bucketSize: 1_001, refillIntervalSeconds: 10 })).toEqual({
			bucketSize: 10,
			refillIntervalMs: 10_000,
		});
	});
});
