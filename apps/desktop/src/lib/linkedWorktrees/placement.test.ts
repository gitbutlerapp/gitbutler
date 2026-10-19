import { workspaceLanes } from "$lib/linkedWorktrees/placement";
import { describe, expect, test } from "vitest";
import type { Segment, Stack, Worktree, WorktreeBase } from "@gitbutler/but-sdk";

function segment(name: string | null, commitIds: string[]): Segment {
	return {
		refName: name === null ? null : { fullNameBytes: [], displayName: name },
		commits: commitIds.map((id) => ({ id })),
	} as unknown as Segment;
}

function worktree(name: string, base: WorktreeBase | null): Worktree {
	return { name, refName: null, head: "f".repeat(40), base, segments: [] };
}

const stacks: Stack[] = [
	{
		id: "stack-1",
		base: null,
		segments: [segment("top", ["c3"]), segment("bottom", ["c2", "c1"])],
	},
	{ id: "stack-2", base: null, segments: [segment(null, ["d1"])] },
];

describe("workspaceLanes", () => {
	test("groups a worktree with the stack holding its base commit", () => {
		const onBottom = worktree("wt-a", { type: "InWorkspace", subject: "c2" });
		const onAnonymous = worktree("wt-b", { type: "InWorkspace", subject: "d1" });
		const lanes = workspaceLanes(stacks, [onBottom, onAnonymous]);

		expect(
			lanes.groups.map((group) =>
				group.lanes.map(({ laneId, baseBranchName, index }) => ({
					laneId,
					baseBranchName,
					index,
				})),
			),
		).toEqual([
			[
				{ laneId: "stack-1", baseBranchName: undefined, index: 0 },
				{ laneId: "worktree:wt-a", baseBranchName: "bottom", index: 1 },
			],
			[
				{ laneId: "stack-2", baseBranchName: undefined, index: 2 },
				{ laneId: "worktree:wt-b", baseBranchName: undefined, index: 3 },
			],
		]);
		expect(lanes.standalone).toEqual([]);
		expect(lanes.count).toBe(4);
	});

	test("puts worktrees resting on nothing a stack shows after the stacks", () => {
		const outside = worktree("wt-outside", { type: "Outside", subject: "t1" });
		const unknown = worktree("wt-unknown", null);
		const belowStacks = worktree("wt-below", { type: "InWorkspace", subject: "t1" });
		const lanes = workspaceLanes(stacks, [outside, unknown, belowStacks]);

		expect(lanes.groups.map((group) => group.lanes.length)).toEqual([1, 1]);
		expect(lanes.standalone.map(({ worktree, index }) => ({ worktree, index }))).toEqual([
			{ worktree: outside, index: 2 },
			{ worktree: unknown, index: 3 },
			{ worktree: belowStacks, index: 4 },
		]);
	});
});
