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

	test("a git object that isn't last meets its guide with a stub", () => {
		expect(
			treeGuides([{ depth: 0 }, { depth: 1, chained: true }, { depth: 1, chained: true }]),
		).toEqual([[], ["stub"], ["last"]]);
	});

	test("a level that has ended draws nothing under the last row", () => {
		// repo › worktree › branch › commit, then a second worktree with a branch.
		expect(
			treeGuides([
				{ depth: 0 },
				{ depth: 1 },
				{ depth: 2, chained: true },
				{ depth: 3, chained: true },
				{ depth: 3, chained: true },
				{ depth: 1 },
				{ depth: 2, chained: true },
			]),
		).toEqual([
			[],
			["line"],
			["line", "last"],
			["line", "none", "stub"],
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
