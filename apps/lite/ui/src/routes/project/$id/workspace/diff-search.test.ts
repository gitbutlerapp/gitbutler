import { diffSearchMatches } from "./diff-search.ts";
import { diffLineTargetFromElement } from "./diff-line-target.ts";
import { hydratePartialDiff, processFile, type CodeViewDiffItem } from "@pierre/diffs";
import { describe, expect, it } from "vitest";

const diffItem = (id: string, patch: string): CodeViewDiffItem<unknown> => {
	const fileDiff = processFile(patch, { cacheKey: id });
	if (!fileDiff) throw new Error("Failed to parse patch");
	return { type: "diff", id, fileDiff };
};

// Two hunks: the first mixes context and changed runs, the second starts at
// different line numbers on either side, so a walk that mixed the sides up
// would report the wrong lines.
const ITEM = diffItem(
	"file.ts",
	[
		"diff --git a/file.ts b/file.ts",
		"--- a/file.ts",
		"+++ b/file.ts",
		"@@ -1,8 +1,9 @@",
		" one",
		"-two",
		"+TWO",
		" three",
		" four",
		"-five",
		"+FIVE",
		"+FIVE AND A HALF",
		" six",
		" seven",
		" eight",
		"@@ -20,5 +21,5 @@",
		" twenty",
		"-twentyone",
		"+TWENTYONE",
		" twentytwo",
		" twentythree",
		" twentyfour",
		"",
	].join("\n"),
);

describe("diffSearchMatches", () => {
	it("finds nothing for an empty query", () => {
		expect(diffSearchMatches([ITEM], "")).toEqual([]);
	});

	it("finds a deleted line on the deletions side with its old line number", () => {
		expect(diffSearchMatches([ITEM], "two")).toContainEqual({
			itemId: "file.ts",
			side: "deletions",
			lineNumber: 2,
		});
	});

	it("finds an added line on the additions side with its new line number", () => {
		expect(diffSearchMatches([ITEM], "FIVE AND A HALF")).toEqual([
			{ itemId: "file.ts", side: "additions", lineNumber: 6 },
		]);
	});

	it("finds a context line once, on the additions side, carrying its deletions-column number", () => {
		expect(diffSearchMatches([ITEM], "four")).toEqual([
			{ itemId: "file.ts", side: "additions", lineNumber: 4, deletionsColumnLine: 4 },
			{ itemId: "file.ts", side: "additions", lineNumber: 25, deletionsColumnLine: 24 },
		]);
	});

	it("matches case-insensitively and reports one match per line and side", () => {
		expect(diffSearchMatches([ITEM], "five")).toEqual([
			{ itemId: "file.ts", side: "deletions", lineNumber: 5 },
			{ itemId: "file.ts", side: "additions", lineNumber: 5 },
			{ itemId: "file.ts", side: "additions", lineNumber: 6 },
		]);
	});

	it("carries line numbers across hunks from each hunk header", () => {
		expect(diffSearchMatches([ITEM], "twentyone")).toEqual([
			{ itemId: "file.ts", side: "deletions", lineNumber: 21 },
			{ itemId: "file.ts", side: "additions", lineNumber: 22 },
		]);
	});

	it("walks files in the order given", () => {
		const other = diffItem(
			"other.ts",
			[
				"diff --git a/other.ts b/other.ts",
				"--- a/other.ts",
				"+++ b/other.ts",
				"@@ -1,1 +1,2 @@",
				" one",
				"+twenty",
				"",
			].join("\n"),
		);

		expect(diffSearchMatches([other, ITEM], "twenty").map((match) => match.itemId)).toEqual([
			"other.ts",
			"file.ts",
			"file.ts",
			"file.ts",
			"file.ts",
			"file.ts",
			"file.ts",
		]);
	});

	it("finds hydrated context only when it has been expanded", () => {
		const partial = diffItem(
			"hydrated.ts",
			[
				"diff --git a/hydrated.ts b/hydrated.ts",
				"--- a/hydrated.ts",
				"+++ b/hydrated.ts",
				"@@ -3,3 +3,3 @@",
				" three",
				"-old",
				"+new",
				" five",
				"",
			].join("\n"),
		);
		const hydrated = hydratePartialDiff("clone", partial.fileDiff, {
			oldFile: { name: "hydrated.ts", contents: "one\ntwo\nthree\nold\nfive\nsix\n" },
			newFile: { name: "hydrated.ts", contents: "one\ntwo\nthree\nnew\nfive\nsix\n" },
		});
		const getSource = () => ({
			fileDiff: hydrated,
			isLineRenderable: (lineNumber: number) => lineNumber === 2,
		});

		expect(diffSearchMatches([partial], "two", getSource)).toEqual([
			{
				itemId: "hydrated.ts",
				side: "additions",
				lineNumber: 2,
				deletionsColumnLine: 2,
			},
		]);
		expect(diffSearchMatches([partial], "six", getSource)).toEqual([]);
	});

	it("recognizes expanded context rows when marking hydrated matches", () => {
		const element = {
			getAttribute: (name: string) =>
				name === "data-line-type" ? "context-expanded" : name === "data-line" ? "2" : null,
			closest: () => null,
		} as unknown as HTMLElement;

		expect(diffLineTargetFromElement({ element, itemId: "hydrated.ts" })).toEqual({
			itemId: "hydrated.ts",
			lineNumber: 2,
			side: "additions",
			lineType: "context",
		});
	});
});
