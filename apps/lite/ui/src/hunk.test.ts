import {
	contiguousSelectionByLine,
	contiguousSelectionsFromHunk,
	rangeFromLineGroups,
	wholeHunkSelectionByLine,
} from "#ui/hunk.ts";
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

const REALIGNED_PATCH = [
	"diff --git a/file.ts b/file.ts",
	"--- a/file.ts",
	"+++ b/file.ts",
	"@@ -1,5 +1,4 @@",
	" let snapshot = snapshot_with_missing_peer_dep_nv();",
	"-let snapshot =",
	"-  NpmResolutionSnapshot::new(snapshot.into_valid().unwrap());",
	"+let snapshot = NpmResolutionSnapshot::new(snapshot.into_valid().unwrap());",
	" let graph = Graph::from_snapshot(snapshot);",
	" assert_eq!(graph.nodes.len(), 3);",
	"",
].join("\n");

const realignedHunk = (() => {
	const parsed = processFile(REALIGNED_PATCH, { cacheKey: "hunk.test.realigned" });
	if (!parsed) throw new Error("Failed to parse realigned patch");
	const hunk = parsed.hunks[0];
	if (!hunk) throw new Error("Realigned patch has no hunk");
	return hunk;
})();

const FORWARD_REALIGNED_PATCH = [
	"diff --git a/file.ts b/file.ts",
	"--- a/file.ts",
	"+++ b/file.ts",
	"@@ -20,1 +28,2 @@",
	"-assert_eq!(graph.nodes.len(), 3);",
	"+let extra = true;",
	"+assert_eq!(graph.nodes.len(), 4);",
	"",
].join("\n");

const forwardRealignedHunk = (() => {
	const parsed = processFile(FORWARD_REALIGNED_PATCH, { cacheKey: "hunk.test.forward-realigned" });
	if (!parsed) throw new Error("Failed to parse forward-realigned patch");
	const hunk = parsed.hunks[0];
	if (!hunk) throw new Error("Forward-realigned patch has no hunk");
	return hunk;
})();

describe("contiguousSelectionsFromHunk", () => {
	it("keeps adjacent similarity-aligned change fragments contiguous", () => {
		expect(realignedHunk.hunkContent.filter(({ type }) => type === "change")).toHaveLength(2);
		expect(contiguousSelectionsFromHunk(realignedHunk).toArray()).toEqual([
			{
				hunkHeader: { oldStart: 1, oldLines: 5, newStart: 1, newLines: 4 },
				lineGroups: [
					{ side: "deletions", start: 2, lines: 2 },
					{ side: "additions", start: 2, lines: 1 },
				],
			},
		]);
	});

	it("keeps the controlled range around a forward-realigned addition", () => {
		expect(forwardRealignedHunk.hunkContent.filter(({ type }) => type === "change")).toEqual([
			expect.objectContaining({ deletions: 0, additions: 1 }),
			expect.objectContaining({ deletions: 1, additions: 1 }),
		]);

		const selection = contiguousSelectionsFromHunk(forwardRealignedHunk).next().value;
		expect(selection?.lineGroups).toEqual([
			{ side: "additions", start: 28, lines: 1 },
			{ side: "deletions", start: 20, lines: 1 },
			{ side: "additions", start: 29, lines: 1 },
		]);
		expect(selection && rangeFromLineGroups(selection.lineGroups)).toEqual({
			start: 28,
			side: "additions",
			end: 29,
		});
	});
});

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
