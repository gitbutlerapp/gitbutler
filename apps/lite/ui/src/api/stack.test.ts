import type { Target } from "@gitbutler/but-sdk";
import { describe, expect, it } from "vitest";
import { pullHasWork } from "#ui/api/stack.ts";

const target = () =>
	({
		refName: { fullNameBytes: [], displayName: "main", remoteName: null },
		commitsAhead: 0,
		upstream: {
			refName: { fullNameBytes: [], displayName: "main", remoteName: "origin" },
			tip: "upstream-tip",
			cutoffMatchesTip: true,
		},
		hasLanesBehind: false,
		canFastForwardLocalTarget: false,
	}) satisfies Target;

describe("pullHasWork", () => {
	it("does not offer pull without an upstream, even for lagging lanes", () => {
		expect(pullHasWork({ ...target(), upstream: null, hasLanesBehind: true })).toBe(false);
	});

	it("does not offer pull when the configured upstream tip is unavailable", () => {
		const value = target();
		expect(
			pullHasWork({
				...value,
				upstream: { ...value.upstream, tip: null, cutoffMatchesTip: false },
				hasLanesBehind: true,
			}),
		).toBe(false);
	});

	it("does not offer pull when everything is current", () => {
		expect(pullHasWork(target())).toBe(false);
	});

	it("offers pull when the stored cutoff differs from upstream", () => {
		const value = target();
		expect(
			pullHasWork({
				...value,
				upstream: { ...value.upstream, cutoffMatchesTip: false },
			}),
		).toBe(true);
	});

	it("offers pull when a lane was left behind an earlier update", () => {
		expect(pullHasWork({ ...target(), hasLanesBehind: true })).toBe(true);
	});

	it("offers pull solely to fast-forward a lagging local target", () => {
		expect(pullHasWork({ ...target(), canFastForwardLocalTarget: true })).toBe(true);
	});
});
