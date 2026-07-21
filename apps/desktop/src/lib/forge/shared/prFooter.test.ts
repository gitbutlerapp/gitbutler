import { updatePrStackInfo, updateStackPrs } from "$lib/forge/shared/prFooter";
import { describe, expect, test, vi } from "vitest";
import type { PrService } from "$lib/forge/prService.svelte";
import type { Segment } from "@gitbutler/but-sdk";

// The desktop receives stack segments child -> parent, i.e. [top, ..., base],
// while the backend expects a single stack ordered bottom-to-top. These tests
// pin that reordering and the target-branch chaining.

function stubPrService() {
	const updateFooters = vi.fn(async () => {});
	const fetch = vi.fn(async (_projectId: string, number: number) => ({
		number,
		body: `body of ${number}`,
	}));
	return {
		service: { fetch, updateFooters } as unknown as PrService,
		updateFooters,
	};
}

function segment(branchName: string, prNumber?: number): Segment {
	return {
		refName: { displayName: branchName },
		metadata: { review: { pullRequest: prNumber } },
	} as unknown as Segment;
}

describe("updatePrStackInfo", () => {
	test("sends the stack bottom-to-top", async () => {
		const { service, updateFooters } = stubPrService();

		await updatePrStackInfo(service, "project", [104, 103, 102], "#");

		expect(updateFooters).toHaveBeenCalledOnce();
		const [, updates] = updateFooters.mock.calls[0] as unknown as [string, { number: number }[]];
		expect(updates.map((u) => u.number)).toEqual([102, 103, 104]);
	});

	test("does nothing for a single PR", async () => {
		const { service, updateFooters } = stubPrService();

		await updatePrStackInfo(service, "project", [104], "#");

		expect(updateFooters).not.toHaveBeenCalled();
	});
});

describe("updateStackPrs", () => {
	test("chains each PR onto the branch below it, bottom-most onto the base", async () => {
		const { service, updateFooters } = stubPrService();
		const topFirst = [segment("top", 3), segment("mid", 2), segment("bottom", 1)];

		await updateStackPrs(service, "project", topFirst, "main", "#");

		expect(updateFooters).toHaveBeenCalledOnce();
		const [, updates] = updateFooters.mock.calls[0] as unknown as [
			string,
			{ number: number; targetBranch: string }[],
		];
		expect(updates).toEqual([
			expect.objectContaining({ number: 1, targetBranch: "main" }),
			expect.objectContaining({ number: 2, targetBranch: "bottom" }),
			expect.objectContaining({ number: 3, targetBranch: "mid" }),
		]);
	});

	test("branches without a PR still act as the target of the PR above", async () => {
		const { service, updateFooters } = stubPrService();
		const topFirst = [segment("top", 3), segment("mid"), segment("bottom", 1)];

		await updateStackPrs(service, "project", topFirst, "main", "#");

		const [, updates] = updateFooters.mock.calls[0] as unknown as [
			string,
			{ number: number; targetBranch: string }[],
		];
		expect(updates).toEqual([
			expect.objectContaining({ number: 1, targetBranch: "main" }),
			expect.objectContaining({ number: 3, targetBranch: "mid" }),
		]);
	});
});
