import {
	contiguousSelectionByLine,
	contiguousSelectionsFromHunk,
	hunkSelectionForLineNavigation,
	lineSelectionsForRange,
	moveSelectedLineRange,
	rangeFromLineGroups,
	selectedLineRangeContainsPoint,
	singleLineSelectionByLine,
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
const unified = { hunks, diffStyle: "unified" } as const;
const split = { hunks, diffStyle: "split" } as const;

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

describe("singleLineSelectionByLine", () => {
	it("selects only the changed line under the cursor", () => {
		expect(singleLineSelectionByLine({ hunks, line: 5, side: "additions" })).toEqual({
			hunkHeader: { oldStart: 1, oldLines: 8, newStart: 1, newLines: 9 },
			lineGroups: [{ side: "additions", start: 5, lines: 1 }],
		});
	});

	it("does not select context", () => {
		expect(singleLineSelectionByLine({ hunks, line: 3, side: "additions" })).toBeNull();
	});
});

describe("lineSelectionsForRange", () => {
	it("recognizes context inside a selected range", () => {
		const range = { start: 2, side: "deletions", end: 6, endSide: "additions" } as const;

		expect(
			selectedLineRangeContainsPoint({
				...unified,
				range,
				line: 3,
				side: "additions",
			}),
		).toBe(true);
		expect(
			selectedLineRangeContainsPoint({
				...unified,
				range,
				line: 7,
				side: "additions",
			}),
		).toBe(false);
	});

	it("compacts an arbitrary unified range", () => {
		expect(
			lineSelectionsForRange({
				...unified,
				range: { start: 2, side: "deletions", end: 6, endSide: "additions" },
			}),
		).toEqual([
			{
				hunkHeader: { oldStart: 1, oldLines: 8, newStart: 1, newLines: 9 },
				lineGroups: [
					{ side: "deletions", start: 2, lines: 1 },
					{ side: "additions", start: 2, lines: 1 },
					{ side: "deletions", start: 5, lines: 1 },
					{ side: "additions", start: 5, lines: 2 },
				],
			},
		]);
	});

	it("keeps checkbox selections line-addressable", () => {
		expect(
			lineSelectionsForRange({
				...unified,
				range: { start: 5, side: "additions", end: 6 },
				granularity: "line",
			}).map(({ lineGroups }) => lineGroups[0]),
		).toEqual([
			{ side: "additions", start: 5, lines: 1 },
			{ side: "additions", start: 6, lines: 1 },
		]);
	});

	it("preserves the same contents for a backwards range", () => {
		const forwards = lineSelectionsForRange({
			...unified,
			range: { start: 2, side: "deletions", end: 6, endSide: "additions" },
		});
		const backwards = lineSelectionsForRange({
			...unified,
			range: { start: 6, side: "additions", end: 2, endSide: "deletions" },
		});

		expect(backwards).toEqual(forwards);
	});

	it("keeps parsed hunks as separate operation selections", () => {
		expect(
			lineSelectionsForRange({
				...unified,
				range: { start: 2, side: "deletions", end: 22, endSide: "additions" },
			}).map(({ hunkHeader }) => hunkHeader),
		).toEqual([
			{ oldStart: 1, oldLines: 8, newStart: 1, newLines: 9 },
			{ oldStart: 20, oldLines: 5, newStart: 21, newLines: 5 },
		]);
	});

	it("includes both changed columns covered by a split range", () => {
		expect(
			lineSelectionsForRange({
				...split,
				range: { start: 2, side: "deletions", end: 5, endSide: "additions" },
			}).flatMap(({ lineGroups }) => lineGroups),
		).toEqual([
			{ side: "deletions", start: 2, lines: 1 },
			{ side: "additions", start: 2, lines: 1 },
			{ side: "deletions", start: 5, lines: 1 },
			{ side: "additions", start: 5, lines: 1 },
		]);
	});
});

describe("moveSelectedLineRange", () => {
	it("moves through each rendered unified line", () => {
		expect(
			moveSelectedLineRange({
				...unified,
				range: { start: 2, side: "deletions", end: 2 },
				offset: 1,
				extend: false,
			}),
		).toEqual({ start: 2, side: "additions", end: 2 });
	});

	it("extends from the mouse range's active end", () => {
		expect(
			moveSelectedLineRange({
				...unified,
				range: { start: 5, side: "additions", end: 2, endSide: "deletions" },
				offset: -1,
				extend: true,
			}),
		).toEqual({ start: 5, side: "additions", end: 1 });
	});

	it("moves by visual row while retaining a split column", () => {
		expect(
			moveSelectedLineRange({
				...split,
				range: { start: 2, side: "deletions", end: 2 },
				offset: 1,
				extend: false,
			}),
		).toEqual({ start: 3, side: "deletions", end: 3 });
	});
});

describe("hunkSelectionForLineNavigation", () => {
	const selections = hunks.flatMap((hunk) => contiguousSelectionsFromHunk(hunk).toArray());

	it("selects the first changed run when moving down from its leading context", () => {
		expect(
			hunkSelectionForLineNavigation({
				...unified,
				selections,
				range: { start: 1, side: "additions", end: 1 },
				offset: 1,
			}),
		).toEqual(selections[0]);
	});

	it("selects the changed run containing the active line before advancing", () => {
		expect(
			hunkSelectionForLineNavigation({
				...unified,
				selections,
				range: { start: 2, side: "deletions", end: 2 },
				offset: 1,
			}),
		).toEqual(selections[0]);
	});

	it.each([
		[-1, 0],
		[1, 1],
	] as const)(
		"selects the immediately adjacent run from context with offset %i",
		(offset, index) => {
			expect(
				hunkSelectionForLineNavigation({
					...unified,
					selections,
					range: { start: 4, side: "additions", end: 4 },
					offset,
				}),
			).toEqual(selections[index]);
		},
	);

	it.each([
		["up", { start: 5, side: "deletions", end: 5 }, -1, 0],
		["down", { start: 2, side: "additions", end: 2 }, 1, 1],
	] as const)("moves %s from a changed run boundary", (_direction, range, offset, index) => {
		expect(
			hunkSelectionForLineNavigation({
				...unified,
				selections,
				range,
				offset,
			}),
		).toEqual(selections[index]);
	});

	it.each([
		["down", { start: 5, side: "deletions", end: 5 }, 1, 1],
		["up", { start: 2, side: "additions", end: 2 }, -1, 0],
	] as const)(
		"selects the current run when moving %s from its boundary",
		(_direction, range, offset, index) => {
			expect(
				hunkSelectionForLineNavigation({
					...unified,
					selections,
					range,
					offset,
				}),
			).toEqual(selections[index]);
		},
	);

	it.each([
		["down", { start: 3, side: "additions", end: 2, endSide: "deletions" }, 1, 1],
		["up", { start: 4, side: "additions", end: 5 }, -1, 0],
	] as const)(
		"moves %s from the directional edge of a backwards range",
		(_direction, range, offset, index) => {
			expect(
				hunkSelectionForLineNavigation({
					...unified,
					selections,
					range,
					offset,
				}),
			).toEqual(selections[index]);
		},
	);
});
