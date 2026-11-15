import { describe, expect, test } from "vitest";
import { treeGuides } from "./treeGuides.ts";

describe("treeGuides", () => {
	test("a top-level row has no guides", () => {
		expect(treeGuides([{ depth: 0 }, { depth: 0 }])).toEqual([[], []]);
	});

	test("the last row under its parent ends its guide", () => {
		expect(treeGuides([{ depth: 0 }, { depth: 1 }, { depth: 1 }])).toEqual([
			[],
			["line"],
			["last"],
		]);
	});

	test("a level that has ended draws nothing under the last row", () => {
		// repo › worktree › branch › commit, then a second worktree with a branch.
		expect(
			treeGuides([
				{ depth: 0 },
				{ depth: 1 },
				{ depth: 2 },
				{ depth: 3 },
				{ depth: 3 },
				{ depth: 1 },
				{ depth: 2 },
			]),
		).toEqual([
			[],
			["line"],
			["line", "last"],
			["line", "none", "line"],
			["line", "none", "last"],
			["last"],
			["none", "last"],
		]);
	});

	test("a shallower row ends the levels under it", () => {
		expect(treeGuides([{ depth: 1 }, { depth: 2 }, { depth: 1 }, { depth: 2 }])).toEqual([
			["line"],
			["line", "last"],
			["last"],
			["none", "last"],
		]);
	});
});
