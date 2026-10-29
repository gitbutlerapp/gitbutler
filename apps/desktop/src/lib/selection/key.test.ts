import {
	checkoutOf,
	createWorktreeSelection,
	key,
	readKey,
	readStableSelectionKey,
	selectionKey,
	stableSelectionKey,
} from "$lib/selection/key";
import { describe, expect, test } from "vitest";

describe("linked worktree selections", () => {
	const id = createWorktreeSelection({ worktree: "feature-wt" });

	test("round-trip through their keys", () => {
		expect(readKey(key({ ...id, path: "src/a.ts" }))).toEqual({
			type: "worktree",
			path: "src/a.ts",
			stackId: undefined,
			worktree: "feature-wt",
		});
		expect(readStableSelectionKey(stableSelectionKey(id))).toEqual({
			type: "worktree",
			stackId: undefined,
			worktree: "feature-wt",
		});
	});

	test("are kept apart from the main checkout's uncommitted changes", () => {
		const unassigned = createWorktreeSelection({ stackId: undefined });
		expect(selectionKey(id)).not.toEqual(selectionKey(unassigned));
		expect(checkoutOf(id)).toEqual({ type: "worktree", subject: "feature-wt" });
		expect(checkoutOf(unassigned)).toEqual({ type: "head" });
	});
});
