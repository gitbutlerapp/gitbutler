import { contiguousSelectionByLine, wholeHunkSelectionByLine } from "#ui/hunk.ts";
import { processFile } from "@pierre/diffs";
import { describe, expect, it } from "vitest";

// Two hunks, the first holding two changed runs with context between them. The second starts at a
// different line on either side, so a lookup that took the wrong side would miss it.
const PATCH = [
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
].join("\n");

const hunks = (() => {
	const parsed = processFile(PATCH, { cacheKey: "hunk.test" });
	if (!parsed) throw new Error("Failed to parse patch");
	return parsed.hunks;
})();

describe("wholeHunkSelectionByLine", () => {
	it("takes every changed run of the hunk holding a context line", () => {
		// " six", the context line between the second changed run and the end of the first hunk.
		expect(wholeHunkSelectionByLine({ hunks, line: 6, side: "deletions" })).toEqual({
			hunkHeader: { oldStart: 1, oldLines: 8, newStart: 1, newLines: 9 },
			lineGroups: [
				{ side: "deletions", start: 2, lines: 1 },
				{ side: "additions", start: 2, lines: 1 },
				{ side: "deletions", start: 5, lines: 1 },
				{ side: "additions", start: 5, lines: 2 },
			],
		});
	});

	it("takes the same hunk from either side of a split diff", () => {
		// The same context line, numbered by the addition column instead.
		expect(wholeHunkSelectionByLine({ hunks, line: 7, side: "additions" })).toEqual(
			wholeHunkSelectionByLine({ hunks, line: 6, side: "deletions" }),
		);
	});

	it("numbers the line by the given side", () => {
		// Line 20 is the second hunk's first line on the deletion side only; the two sides drifted
		// apart by the line the first hunk added.
		expect(wholeHunkSelectionByLine({ hunks, line: 20, side: "deletions" })?.hunkHeader).toEqual({
			oldStart: 20,
			oldLines: 5,
			newStart: 21,
			newLines: 5,
		});
		expect(wholeHunkSelectionByLine({ hunks, line: 20, side: "additions" })).toBeNull();
	});

	it("has no hunk for a line between them", () => {
		expect(wholeHunkSelectionByLine({ hunks, line: 15, side: "deletions" })).toBeNull();
	});

	it("takes more than the changed line under it does", () => {
		expect(contiguousSelectionByLine({ hunks, line: 2, side: "deletions" })).toEqual({
			hunkHeader: { oldStart: 1, oldLines: 8, newStart: 1, newLines: 9 },
			lineGroups: [
				{ side: "deletions", start: 2, lines: 1 },
				{ side: "additions", start: 2, lines: 1 },
			],
		});
	});

	// Why the two live side by side: a context line names a hunk but no changed run in it.
	it("answers where the changed-run lookup cannot", () => {
		expect(contiguousSelectionByLine({ hunks, line: 6, side: "deletions" })).toBeNull();
		expect(wholeHunkSelectionByLine({ hunks, line: 6, side: "deletions" })).not.toBeNull();
	});
});
