import {
	lineIdsToHunkHeaders,
	extractLineGroups,
	extractAllGroups,
	hunkContainsHunk,
	hunkContainsLine,
	getLineLocks,
	orderHeaders,
	diffToHunkHeaders
} from '$lib/hunks/hunk';
import { describe, expect, test } from 'vitest';
import type { LineId } from '@gitbutler/ui/utils/diffParsing';

describe.concurrent('lineIdsToHunkHeaders', () => {
	test('should return empty array when given no line IDs', () => {
		expect(lineIdsToHunkHeaders([], '', 'discard')).toEqual([]);
		expect(lineIdsToHunkHeaders([], '', 'commit')).toEqual([]);
	});

	test('should return a single hunk header when given a single line ID', () => {
		const lineIds = [{ oldLine: 2, newLine: undefined }];
		const hunkDiff = `@@ -1,3 +1,2 @@
  line 1
- line 2
  line 3
`;
		expect(lineIdsToHunkHeaders(lineIds, hunkDiff, 'discard')).toEqual([
			{ oldStart: 2, oldLines: 1, newStart: 1, newLines: 2 }
		]);
		expect(lineIdsToHunkHeaders(lineIds, hunkDiff, 'commit')).toEqual([
			{ oldStart: 2, oldLines: 1, newStart: 0, newLines: 0 }
		]);
	});

	test('can deal with a big diff and a neat selection', () => {
		const hunkDiff = `@@ -1,10 +1,12 @@
 1
 2
 3
- 4
+ new 4
 5
- 6
- 7
+ new 6
+ new 7
+ an extra line
+ another extra line
 8
 9
 10
`;
		const lineIds = [
			{ oldLine: 4, newLine: undefined }, // 4
			{ oldLine: undefined, newLine: 6 }, // new 6
			{ oldLine: undefined, newLine: 7 } // new 7
		];
		expect(lineIdsToHunkHeaders(lineIds, hunkDiff, 'discard')).toEqual([
			{ oldStart: 4, oldLines: 1, newStart: 1, newLines: 12 },
			{ oldStart: 1, oldLines: 10, newStart: 6, newLines: 2 }
		]);
		expect(lineIdsToHunkHeaders(lineIds, hunkDiff, 'commit')).toEqual([
			{ oldStart: 4, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 6, newLines: 2 }
		]);
	});

	test('can deal with a big diff and an overlapping selection', () => {
		const hunkDiff = `@@ -1,10 +1,12 @@
 1
 2
 3
- 4
+ new 4
 5
- 6
- 7
+ new 6
+ new 7
+ an extra line
+ another extra line
 8
 9
 10
`;
		const lineIds = [
			{ oldLine: 4, newLine: undefined }, // 4
			{ oldLine: undefined, newLine: 4 }, // new 4
			{ oldLine: 6, newLine: undefined }, // 6
			{ oldLine: undefined, newLine: 6 }, // new 6
			{ oldLine: undefined, newLine: 7 } // new 7
		];
		expect(lineIdsToHunkHeaders(lineIds, hunkDiff, 'discard')).toEqual([
			{ oldStart: 4, oldLines: 1, newStart: 1, newLines: 12 },
			{ oldStart: 1, oldLines: 10, newStart: 4, newLines: 1 },
			{ oldStart: 6, oldLines: 1, newStart: 1, newLines: 12 },
			{ oldStart: 1, oldLines: 10, newStart: 6, newLines: 2 }
		]);
		expect(lineIdsToHunkHeaders(lineIds, hunkDiff, 'commit')).toEqual([
			{ oldStart: 4, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 4, newLines: 1 },
			{ oldStart: 6, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 6, newLines: 2 }
		]);
	});

	test('can deal with a big diff and an overlapping selection, unordered', () => {
		const hunkDiff = `@@ -1,10 +1,12 @@
 1
 2
 3
- 4
+ new 4
 5
- 6
- 7
+ new 6
+ new 7
+ an extra line
+ another extra line
 8
 9
 10
`;
		const lineIds = [
			{ oldLine: undefined, newLine: 7 }, // new 7
			{ oldLine: undefined, newLine: 4 }, // new 4
			{ oldLine: 6, newLine: undefined }, // 6
			{ oldLine: undefined, newLine: 6 }, // new 6
			{ oldLine: 4, newLine: undefined } // 4
		];
		expect(lineIdsToHunkHeaders(lineIds, hunkDiff, 'discard')).toEqual([
			{ oldStart: 4, oldLines: 1, newStart: 1, newLines: 12 },
			{ oldStart: 1, oldLines: 10, newStart: 4, newLines: 1 },
			{ oldStart: 6, oldLines: 1, newStart: 1, newLines: 12 },
			{ oldStart: 1, oldLines: 10, newStart: 6, newLines: 2 }
		]);
		expect(lineIdsToHunkHeaders(lineIds, hunkDiff, 'commit')).toEqual([
			{ oldStart: 4, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 4, newLines: 1 },
			{ oldStart: 6, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 6, newLines: 2 }
		]);
	});
});

describe.concurrent('extractLineGroups', () => {
	test('should return an empty array when given no line IDs', () => {
		const hunkDiff = `@@ -1,4 +1,2 @@
- line 1
- line 2
+ new line 1
+ new line 2
- line 3
- line 4
`;
		expect(extractLineGroups([], hunkDiff)).toEqual([
			[],
			{
				oldStart: 1,
				oldLines: 4,
				newStart: 1,
				newLines: 2
			}
		]);
	});

	test('should return the correct line groups for each type', () => {
		const lineIds: LineId[] = [
			{ oldLine: 1, newLine: undefined },
			{ oldLine: 2, newLine: undefined },
			{ oldLine: undefined, newLine: 1 },
			{ oldLine: undefined, newLine: 2 },
			{ oldLine: 3, newLine: undefined },
			{ oldLine: 4, newLine: undefined }
		];

		const hunkDiff = `@@ -1,4 +1,2 @@
- line 1
- line 2
+ new line 1
+ new line 2
- line 3
- line 4
`;

		expect(extractLineGroups(lineIds, hunkDiff)).toEqual([
			[
				{ type: 'removed', lines: [lineIds[0], lineIds[1]] },
				{ type: 'added', lines: [lineIds[2], lineIds[3]] },
				{ type: 'removed', lines: [lineIds[4], lineIds[5]] }
			],
			{
				oldStart: 1,
				oldLines: 4,
				newStart: 1,
				newLines: 2
			}
		]);
	});

	test('should be able to deal with non-consecutive line numbers', () => {
		const lineIds: LineId[] = [
			{ oldLine: 1, newLine: undefined },
			{ oldLine: 3, newLine: undefined },
			{ oldLine: undefined, newLine: 2 },
			{ oldLine: 4, newLine: undefined }
		];

		const hunkDiff = `@@ -1,4 +1,2 @@
- line 1
line 2
- line 3
+ new line 2
- line 4
`;

		expect(extractLineGroups(lineIds, hunkDiff)).toEqual([
			[
				{ type: 'removed', lines: [lineIds[0]] },
				{ type: 'removed', lines: [lineIds[1]] },
				{ type: 'added', lines: [lineIds[2]] },
				{ type: 'removed', lines: [lineIds[3]] }
			],
			{
				oldStart: 1,
				oldLines: 4,
				newStart: 1,
				newLines: 2
			}
		]);
	});

	test('should be able to deal with non-consecutive, out of orderline numbers', () => {
		const lineIds: LineId[] = [
			{ oldLine: 3, newLine: undefined },
			{ oldLine: 4, newLine: undefined },
			{ oldLine: undefined, newLine: 2 },
			{ oldLine: 1, newLine: undefined }
		];

		const hunkDiff = `@@ -1,4 +1,2 @@
- line 1
line 2
- line 3
+ new line 2
- line 4
`;

		expect(extractLineGroups(lineIds, hunkDiff)).toEqual([
			[
				{ type: 'removed', lines: [lineIds[3]] },
				{ type: 'removed', lines: [lineIds[0]] },
				{ type: 'added', lines: [lineIds[2]] },
				{ type: 'removed', lines: [lineIds[1]] }
			],
			{
				oldStart: 1,
				oldLines: 4,
				newStart: 1,
				newLines: 2
			}
		]);
	});
});

describe('hunkContainsHunk', () => {
	const baseHunk = {
		oldStart: 10,
		oldLines: 10,
		newStart: 20,
		newLines: 10,
		diff: ''
	};
	test('returns true if hunk b is fully inside hunk a', () => {
		const inner = { ...baseHunk, oldStart: 12, oldLines: 5, newStart: 22, newLines: 5, diff: '' };
		expect(hunkContainsHunk(baseHunk, inner)).toBe(true);
	});
	test('returns false if hunk b is not fully inside hunk a', () => {
		const outer = { ...baseHunk, oldStart: 5, oldLines: 20, newStart: 15, newLines: 20, diff: '' };
		expect(hunkContainsHunk(baseHunk, outer)).toBe(false);
	});
	test('returns true when hunk b ends at the exact same line as hunk a', () => {
		// Hunk a: oldStart=10, oldLines=10 -> covers old lines 10-19
		// Hunk b: oldStart=15, oldLines=5 -> covers old lines 15-19 (ends at same line)
		const inner = { oldStart: 15, oldLines: 5, newStart: 25, newLines: 5, diff: '' };
		expect(hunkContainsHunk(baseHunk, inner)).toBe(true);
	});
	test('returns false when hunk b extends beyond hunk a by one line', () => {
		// Hunk a: oldStart=10, oldLines=10 -> covers old lines 10-19
		// Hunk b: oldStart=15, oldLines=6 -> covers old lines 15-20 (extends beyond by 1)
		const extending = {
			oldStart: 15,
			oldLines: 6,
			newStart: 25,
			newLines: 6,
			diff: ''
		};
		expect(hunkContainsHunk(baseHunk, extending)).toBe(false);
	});
	test('returns true when ranges are identical', () => {
		expect(hunkContainsHunk(baseHunk, baseHunk)).toBe(true);
	});
});

describe('hunkContainsLine', () => {
	const hunk = { oldStart: 5, oldLines: 5, newStart: 10, newLines: 5, diff: '' };
	test('returns true for a line inside the hunk (old line)', () => {
		expect(hunkContainsLine(hunk, { oldLine: 7, newLine: undefined })).toBe(true);
	});
	test('returns true for a line inside the hunk (new line)', () => {
		expect(hunkContainsLine(hunk, { oldLine: undefined, newLine: 12 })).toBe(true);
	});
	test('returns false for a line outside the hunk', () => {
		expect(hunkContainsLine(hunk, { oldLine: 20, newLine: undefined })).toBe(false);
	});
	test('returns true for a line with both old and new inside', () => {
		expect(hunkContainsLine(hunk, { oldLine: 6, newLine: 11 })).toBe(true);
	});
	test('returns false for a line with both old and new outside', () => {
		expect(hunkContainsLine(hunk, { oldLine: 1, newLine: 1 })).toBe(false);
	});
});

describe('getLineLocks', () => {
	const diff = `@@ -1,3 +1,3 @@\n line 1\n-line 2\n+line 2 changed\n line 3`;
	const hunk = { oldStart: 1, oldLines: 3, newStart: 1, newLines: 3, diff };
	const locks = [
		{
			hunk: { oldStart: 2, oldLines: 1, newStart: 2, newLines: 1, diff },
			locks: [{ stackId: 'stack1', commitId: 'commit1' }]
		}
	];
	test('returns line locks for lines covered by locks', () => {
		const [fullyLocked, lineLocks] = getLineLocks(hunk, locks);
		expect(fullyLocked).toBe(true);
		expect(lineLocks).toEqual([
			{ oldLine: 2, newLine: undefined, locks: [{ stackId: 'stack1', commitId: 'commit1' }] },
			{ oldLine: undefined, newLine: 2, locks: [{ stackId: 'stack1', commitId: 'commit1' }] }
		]);
	});
	test('returns empty array if no locks match', () => {
		const noLocks = [
			{
				hunk: { oldStart: 10, oldLines: 1, newStart: 10, newLines: 1, diff: '' },
				locks: [{ stackId: 'stack2', commitId: 'commit2' }]
			}
		];
		const [fullyLocked, lineLocks] = getLineLocks(hunk, noLocks);
		expect(fullyLocked).toBe(false);
		expect(lineLocks).toEqual([]);
	});

	test('returns partially locked for hunks with only some lines covered', () => {
		// Diff with three changed lines (lines 2, 3, 4)
		const partialDiff = `@@ -1,5 +1,5 @@\n line 1\n-line 2\n-line 3\n-line 4\n+line 2 changed\n+line 3 changed\n+line 4 changed\n line 5`;
		const partialHunk = { oldStart: 1, oldLines: 5, newStart: 1, newLines: 5, diff: partialDiff };
		const partialLocks = [
			{
				hunk: { oldStart: 3, oldLines: 1, newStart: 3, newLines: 1, diff: partialDiff },
				locks: [{ stackId: 'stack1', commitId: 'commit1' }]
			}
		];
		// Only line 3 is locked, lines 2 and 4 are not
		const [fullyLocked, lineLocks] = getLineLocks(partialHunk, partialLocks);
		expect(fullyLocked).toBe(false);
		expect(lineLocks).toEqual([
			{ oldLine: 3, newLine: undefined, locks: [{ stackId: 'stack1', commitId: 'commit1' }] },
			{ oldLine: undefined, newLine: 3, locks: [{ stackId: 'stack1', commitId: 'commit1' }] }
		]);
	});
});

describe('orderHeaders', () => {
	test('should properly order the headers', () => {
		const headers = [
			{
				oldStart: 0,
				oldLines: 0,
				newStart: 3,
				newLines: 1
			},
			{
				oldStart: 0,
				oldLines: 0,
				newStart: 5,
				newLines: 1
			},
			{
				oldStart: 3,
				oldLines: 1,
				newStart: 0,
				newLines: 0
			},
			{
				oldStart: 5,
				oldLines: 1,
				newStart: 0,
				newLines: 0
			}
		];

		const orderedHeaders = headers.sort(orderHeaders);

		expect(orderedHeaders).toEqual([
			{
				oldStart: 0,
				oldLines: 0,
				newStart: 3,
				newLines: 1
			},
			{
				oldStart: 3,
				oldLines: 1,
				newStart: 0,
				newLines: 0
			},
			{
				oldStart: 0,
				oldLines: 0,
				newStart: 5,
				newLines: 1
			},
			{
				oldStart: 5,
				oldLines: 1,
				newStart: 0,
				newLines: 0
			}
		]);
	});

	test('should order headers with mixed zeroed and non-zeroed starts', () => {
		const headers = [
			{ oldStart: 0, oldLines: 0, newStart: 10, newLines: 2 },
			{ oldStart: 2, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 1, newLines: 1 },
			{ oldStart: 5, oldLines: 2, newStart: 0, newLines: 0 }
		];
		const ordered = headers.sort(orderHeaders);
		expect(ordered).toEqual([
			{ oldStart: 0, oldLines: 0, newStart: 1, newLines: 1 },
			{ oldStart: 2, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 5, oldLines: 2, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 10, newLines: 2 }
		]);
	});

	test('should order headers with all oldStart zeroed', () => {
		const headers = [
			{ oldStart: 0, oldLines: 0, newStart: 8, newLines: 1 },
			{ oldStart: 0, oldLines: 0, newStart: 2, newLines: 1 },
			{ oldStart: 0, oldLines: 0, newStart: 5, newLines: 1 }
		];
		const ordered = headers.sort(orderHeaders);
		expect(ordered).toEqual([
			{ oldStart: 0, oldLines: 0, newStart: 2, newLines: 1 },
			{ oldStart: 0, oldLines: 0, newStart: 5, newLines: 1 },
			{ oldStart: 0, oldLines: 0, newStart: 8, newLines: 1 }
		]);
	});

	test('should order headers with all newStart zeroed', () => {
		const headers = [
			{ oldStart: 7, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 2, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 5, oldLines: 1, newStart: 0, newLines: 0 }
		];
		const ordered = headers.sort(orderHeaders);
		expect(ordered).toEqual([
			{ oldStart: 2, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 5, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 7, oldLines: 1, newStart: 0, newLines: 0 }
		]);
	});

	test('should handle headers with both starts zeroed (should remain stable)', () => {
		const headers = [
			{ oldStart: 0, oldLines: 0, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 0, newLines: 0 }
		];
		const ordered = headers.sort(orderHeaders);
		expect(ordered).toEqual([
			{ oldStart: 0, oldLines: 0, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 0, newLines: 0 }
		]);
	});

	test('should order headers with negative values', () => {
		const headers = [
			{ oldStart: -2, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: -5, newLines: 1 },
			{ oldStart: 1, oldLines: 1, newStart: 0, newLines: 0 }
		];
		const ordered = headers.sort(orderHeaders);
		expect(ordered).toEqual([
			{ oldStart: 0, oldLines: 0, newStart: -5, newLines: 1 },
			{ oldStart: -2, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 1, oldLines: 1, newStart: 0, newLines: 0 }
		]);
	});
});

describe.concurrent('extractAllGroups', () => {
	test('should extract all added and removed lines from a simple diff', () => {
		const hunkDiff = `@@ -1,3 +1,3 @@
 line 1
-line 2
+line 2 changed
 line 3
`;
		const [lineGroups, parentHunkHeader] = extractAllGroups(hunkDiff);

		expect(parentHunkHeader).toEqual({
			oldStart: 1,
			oldLines: 3,
			newStart: 1,
			newLines: 3
		});

		expect(lineGroups).toEqual([
			{
				type: 'removed',
				lines: [{ oldLine: 2, newLine: undefined }]
			},
			{
				type: 'added',
				lines: [{ oldLine: undefined, newLine: 2 }]
			}
		]);
	});

	test('should extract multiple consecutive removed lines', () => {
		const hunkDiff = `@@ -1,4 +1,3 @@
 line 1
-line 2
-line 3
-line 4
+new line 2
+new line 3
`;
		const [lineGroups, parentHunkHeader] = extractAllGroups(hunkDiff);

		expect(parentHunkHeader).toEqual({
			oldStart: 1,
			oldLines: 4,
			newStart: 1,
			newLines: 3
		});

		expect(lineGroups).toEqual([
			{
				type: 'removed',
				lines: [
					{ oldLine: 2, newLine: undefined },
					{ oldLine: 3, newLine: undefined },
					{ oldLine: 4, newLine: undefined }
				]
			},
			{
				type: 'added',
				lines: [
					{ oldLine: undefined, newLine: 2 },
					{ oldLine: undefined, newLine: 3 }
				]
			}
		]);
	});

	test('should extract multiple consecutive added lines', () => {
		const hunkDiff = `@@ -1,3 +1,4 @@
 line 1
-old line
+new line 2
+new line 3
+new line 4
 line 5
`;
		const [lineGroups, parentHunkHeader] = extractAllGroups(hunkDiff);

		expect(parentHunkHeader).toEqual({
			oldStart: 1,
			oldLines: 3,
			newStart: 1,
			newLines: 4
		});

		expect(lineGroups).toEqual([
			{
				type: 'removed',
				lines: [{ oldLine: 2, newLine: undefined }]
			},
			{
				type: 'added',
				lines: [
					{ oldLine: undefined, newLine: 2 },
					{ oldLine: undefined, newLine: 3 },
					{ oldLine: undefined, newLine: 4 }
				]
			}
		]);
	});

	test('should group non-consecutive changes separately', () => {
		const hunkDiff = `@@ -1,6 +1,6 @@
 line 1
-line 2
+line 2 changed
 line 3
 line 4
-line 5
+line 5 changed
 line 6
`;
		const [lineGroups, parentHunkHeader] = extractAllGroups(hunkDiff);

		expect(parentHunkHeader).toEqual({
			oldStart: 1,
			oldLines: 6,
			newStart: 1,
			newLines: 6
		});

		expect(lineGroups).toEqual([
			{
				type: 'removed',
				lines: [{ oldLine: 2, newLine: undefined }]
			},
			{
				type: 'added',
				lines: [{ oldLine: undefined, newLine: 2 }]
			},
			{
				type: 'removed',
				lines: [{ oldLine: 5, newLine: undefined }]
			},
			{
				type: 'added',
				lines: [{ oldLine: undefined, newLine: 5 }]
			}
		]);
	});

	test('should handle diff with only added lines', () => {
		const hunkDiff = `@@ -1,2 +1,4 @@
 line 1
+new line 2
+new line 3
 line 4
`;
		const [lineGroups, parentHunkHeader] = extractAllGroups(hunkDiff);

		expect(parentHunkHeader).toEqual({
			oldStart: 1,
			oldLines: 2,
			newStart: 1,
			newLines: 4
		});

		expect(lineGroups).toEqual([
			{
				type: 'added',
				lines: [
					{ oldLine: undefined, newLine: 2 },
					{ oldLine: undefined, newLine: 3 }
				]
			}
		]);
	});

	test('should handle diff with only removed lines', () => {
		const hunkDiff = `@@ -1,4 +1,2 @@
 line 1
-line 2
-line 3
 line 4
`;
		const [lineGroups, parentHunkHeader] = extractAllGroups(hunkDiff);

		expect(parentHunkHeader).toEqual({
			oldStart: 1,
			oldLines: 4,
			newStart: 1,
			newLines: 2
		});

		expect(lineGroups).toEqual([
			{
				type: 'removed',
				lines: [
					{ oldLine: 2, newLine: undefined },
					{ oldLine: 3, newLine: undefined }
				]
			}
		]);
	});

	test('should handle diff with only context lines (no changes)', () => {
		const hunkDiff = `@@ -1,3 +1,3 @@
 line 1
 line 2
 line 3
`;
		const [lineGroups, parentHunkHeader] = extractAllGroups(hunkDiff);

		expect(parentHunkHeader).toEqual({
			oldStart: 1,
			oldLines: 3,
			newStart: 1,
			newLines: 3
		});

		expect(lineGroups).toEqual([]);
	});

	test('should handle complex diff with multiple change groups', () => {
		const hunkDiff = `@@ -1,10 +1,12 @@
 1
 2
 3
- 4
+ new 4
 5
- 6
- 7
+ new 6
+ new 7
+ an extra line
+ another extra line
 8
 9
 10
`;
		const [lineGroups, parentHunkHeader] = extractAllGroups(hunkDiff);

		expect(parentHunkHeader).toEqual({
			oldStart: 1,
			oldLines: 10,
			newStart: 1,
			newLines: 12
		});

		expect(lineGroups).toEqual([
			{
				type: 'removed',
				lines: [{ oldLine: 4, newLine: undefined }]
			},
			{
				type: 'added',
				lines: [{ oldLine: undefined, newLine: 4 }]
			},
			{
				type: 'removed',
				lines: [
					{ oldLine: 6, newLine: undefined },
					{ oldLine: 7, newLine: undefined }
				]
			},
			{
				type: 'added',
				lines: [
					{ oldLine: undefined, newLine: 6 },
					{ oldLine: undefined, newLine: 7 },
					{ oldLine: undefined, newLine: 8 },
					{ oldLine: undefined, newLine: 9 }
				]
			}
		]);
	});

	test('should handle file deletion (all lines removed)', () => {
		const hunkDiff = `@@ -1,3 +0,0 @@
-line 1
-line 2
-line 3
`;
		const [lineGroups, parentHunkHeader] = extractAllGroups(hunkDiff);

		expect(parentHunkHeader).toEqual({
			oldStart: 1,
			oldLines: 3,
			newStart: 0,
			newLines: 0
		});

		expect(lineGroups).toEqual([
			{
				type: 'removed',
				lines: [
					{ oldLine: 1, newLine: undefined },
					{ oldLine: 2, newLine: undefined },
					{ oldLine: 3, newLine: undefined }
				]
			}
		]);
	});

	test('should handle new file creation (all lines added)', () => {
		const hunkDiff = `@@ -0,0 +1,3 @@
+line 1
+line 2
+line 3
`;
		const [lineGroups, parentHunkHeader] = extractAllGroups(hunkDiff);

		expect(parentHunkHeader).toEqual({
			oldStart: 0,
			oldLines: 0,
			newStart: 1,
			newLines: 3
		});

		expect(lineGroups).toEqual([
			{
				type: 'added',
				lines: [
					{ oldLine: undefined, newLine: 1 },
					{ oldLine: undefined, newLine: 2 },
					{ oldLine: undefined, newLine: 3 }
				]
			}
		]);
	});
});

describe.concurrent('diffToHunkHeaders', () => {
	test('should return empty array for diff with no changes', () => {
		const hunkDiff = `@@ -1,3 +1,3 @@
 line 1
 line 2
 line 3
`;
		expect(diffToHunkHeaders(hunkDiff, 'discard')).toEqual([]);
		expect(diffToHunkHeaders(hunkDiff, 'commit')).toEqual([]);
	});

	test('should convert single removed line to hunk header for discard action', () => {
		const hunkDiff = `@@ -1,3 +1,2 @@
 line 1
-line 2
 line 3
`;
		expect(diffToHunkHeaders(hunkDiff, 'discard')).toEqual([
			{ oldStart: 2, oldLines: 1, newStart: 1, newLines: 2 }
		]);
	});

	test('should convert single removed line to hunk header for commit action', () => {
		const hunkDiff = `@@ -1,3 +1,2 @@
 line 1
-line 2
 line 3
`;
		expect(diffToHunkHeaders(hunkDiff, 'commit')).toEqual([
			{ oldStart: 2, oldLines: 1, newStart: 0, newLines: 0 }
		]);
	});

	test('should convert single added line to hunk header for discard action', () => {
		const hunkDiff = `@@ -1,2 +1,3 @@
 line 1
+new line 2
 line 3
`;
		expect(diffToHunkHeaders(hunkDiff, 'discard')).toEqual([
			{ oldStart: 1, oldLines: 2, newStart: 2, newLines: 1 }
		]);
	});

	test('should convert single added line to hunk header for commit action', () => {
		const hunkDiff = `@@ -1,2 +1,3 @@
 line 1
+new line 2
 line 3
`;
		expect(diffToHunkHeaders(hunkDiff, 'commit')).toEqual([
			{ oldStart: 0, oldLines: 0, newStart: 2, newLines: 1 }
		]);
	});

	test('should convert multiple consecutive removed lines', () => {
		const hunkDiff = `@@ -1,5 +1,2 @@
 line 1
-line 2
-line 3
-line 4
 line 5
`;
		expect(diffToHunkHeaders(hunkDiff, 'discard')).toEqual([
			{ oldStart: 2, oldLines: 3, newStart: 1, newLines: 2 }
		]);
		expect(diffToHunkHeaders(hunkDiff, 'commit')).toEqual([
			{ oldStart: 2, oldLines: 3, newStart: 0, newLines: 0 }
		]);
	});

	test('should convert multiple consecutive added lines', () => {
		const hunkDiff = `@@ -1,2 +1,5 @@
 line 1
+new line 2
+new line 3
+new line 4
 line 5
`;
		expect(diffToHunkHeaders(hunkDiff, 'discard')).toEqual([
			{ oldStart: 1, oldLines: 2, newStart: 2, newLines: 3 }
		]);
		expect(diffToHunkHeaders(hunkDiff, 'commit')).toEqual([
			{ oldStart: 0, oldLines: 0, newStart: 2, newLines: 3 }
		]);
	});

	test('should convert mixed add/remove to separate hunk headers', () => {
		const hunkDiff = `@@ -1,3 +1,3 @@
 line 1
-line 2
+line 2 changed
 line 3
`;
		expect(diffToHunkHeaders(hunkDiff, 'discard')).toEqual([
			{ oldStart: 2, oldLines: 1, newStart: 1, newLines: 3 },
			{ oldStart: 1, oldLines: 3, newStart: 2, newLines: 1 }
		]);
		expect(diffToHunkHeaders(hunkDiff, 'commit')).toEqual([
			{ oldStart: 2, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 2, newLines: 1 }
		]);
	});

	test('should handle multiple separate change groups', () => {
		const hunkDiff = `@@ -1,6 +1,6 @@
 line 1
-line 2
+line 2 changed
 line 3
 line 4
-line 5
+line 5 changed
 line 6
`;
		expect(diffToHunkHeaders(hunkDiff, 'discard')).toEqual([
			{ oldStart: 2, oldLines: 1, newStart: 1, newLines: 6 },
			{ oldStart: 1, oldLines: 6, newStart: 2, newLines: 1 },
			{ oldStart: 5, oldLines: 1, newStart: 1, newLines: 6 },
			{ oldStart: 1, oldLines: 6, newStart: 5, newLines: 1 }
		]);
		expect(diffToHunkHeaders(hunkDiff, 'commit')).toEqual([
			{ oldStart: 2, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 2, newLines: 1 },
			{ oldStart: 5, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 5, newLines: 1 }
		]);
	});

	test('should handle complex diff with multiple change types', () => {
		const hunkDiff = `@@ -1,10 +1,12 @@
 1
 2
 3
- 4
+ new 4
 5
- 6
- 7
+ new 6
+ new 7
+ an extra line
+ another extra line
 8
 9
 10
`;
		expect(diffToHunkHeaders(hunkDiff, 'discard')).toEqual([
			{ oldStart: 4, oldLines: 1, newStart: 1, newLines: 12 },
			{ oldStart: 1, oldLines: 10, newStart: 4, newLines: 1 },
			{ oldStart: 6, oldLines: 2, newStart: 1, newLines: 12 },
			{ oldStart: 1, oldLines: 10, newStart: 6, newLines: 4 }
		]);
		expect(diffToHunkHeaders(hunkDiff, 'commit')).toEqual([
			{ oldStart: 4, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 4, newLines: 1 },
			{ oldStart: 6, oldLines: 2, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 6, newLines: 4 }
		]);
	});

	test('should handle file deletion (all lines removed)', () => {
		const hunkDiff = `@@ -1,3 +0,0 @@
-line 1
-line 2
-line 3
`;
		expect(diffToHunkHeaders(hunkDiff, 'discard')).toEqual([
			{ oldStart: 1, oldLines: 3, newStart: 0, newLines: 0 }
		]);
		expect(diffToHunkHeaders(hunkDiff, 'commit')).toEqual([
			{ oldStart: 1, oldLines: 3, newStart: 0, newLines: 0 }
		]);
	});

	test('should handle new file creation (all lines added)', () => {
		const hunkDiff = `@@ -0,0 +1,3 @@
+line 1
+line 2
+line 3
`;
		expect(diffToHunkHeaders(hunkDiff, 'discard')).toEqual([
			{ oldStart: 0, oldLines: 0, newStart: 1, newLines: 3 }
		]);
		expect(diffToHunkHeaders(hunkDiff, 'commit')).toEqual([
			{ oldStart: 0, oldLines: 0, newStart: 1, newLines: 3 }
		]);
	});

	test('should handle diff with only removals', () => {
		const hunkDiff = `@@ -1,5 +1,2 @@
 line 1
-line 2
-line 3
-line 4
 line 5
`;
		expect(diffToHunkHeaders(hunkDiff, 'discard')).toEqual([
			{ oldStart: 2, oldLines: 3, newStart: 1, newLines: 2 }
		]);
		expect(diffToHunkHeaders(hunkDiff, 'commit')).toEqual([
			{ oldStart: 2, oldLines: 3, newStart: 0, newLines: 0 }
		]);
	});

	test('should handle diff with only additions', () => {
		const hunkDiff = `@@ -1,2 +1,5 @@
 line 1
+new line 2
+new line 3
+new line 4
 line 5
`;
		expect(diffToHunkHeaders(hunkDiff, 'discard')).toEqual([
			{ oldStart: 1, oldLines: 2, newStart: 2, newLines: 3 }
		]);
		expect(diffToHunkHeaders(hunkDiff, 'commit')).toEqual([
			{ oldStart: 0, oldLines: 0, newStart: 2, newLines: 3 }
		]);
	});

	test('should handle diff at the beginning of file', () => {
		const hunkDiff = `@@ -1,3 +1,4 @@
-old first line
+new first line
+another new line
 line 2
 line 3
`;
		expect(diffToHunkHeaders(hunkDiff, 'discard')).toEqual([
			{ oldStart: 1, oldLines: 1, newStart: 1, newLines: 4 },
			{ oldStart: 1, oldLines: 3, newStart: 1, newLines: 2 }
		]);
		expect(diffToHunkHeaders(hunkDiff, 'commit')).toEqual([
			{ oldStart: 1, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 1, newLines: 2 }
		]);
	});

	test('should handle diff at the end of file', () => {
		const hunkDiff = `@@ -1,3 +1,4 @@
 line 1
 line 2
-old last line
+new last line
+another new line
`;
		expect(diffToHunkHeaders(hunkDiff, 'discard')).toEqual([
			{ oldStart: 3, oldLines: 1, newStart: 1, newLines: 4 },
			{ oldStart: 1, oldLines: 3, newStart: 3, newLines: 2 }
		]);
		expect(diffToHunkHeaders(hunkDiff, 'commit')).toEqual([
			{ oldStart: 3, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 3, newLines: 2 }
		]);
	});

	test('should handle alternating additions and removals', () => {
		const hunkDiff = `@@ -1,5 +1,5 @@
 line 1
-line 2
+new line 2
-line 3
+new line 3
 line 5
`;
		expect(diffToHunkHeaders(hunkDiff, 'discard')).toEqual([
			{ oldStart: 2, oldLines: 1, newStart: 1, newLines: 5 },
			{ oldStart: 1, oldLines: 5, newStart: 2, newLines: 1 },
			{ oldStart: 3, oldLines: 1, newStart: 1, newLines: 5 },
			{ oldStart: 1, oldLines: 5, newStart: 3, newLines: 1 }
		]);
		expect(diffToHunkHeaders(hunkDiff, 'commit')).toEqual([
			{ oldStart: 2, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 2, newLines: 1 },
			{ oldStart: 3, oldLines: 1, newStart: 0, newLines: 0 },
			{ oldStart: 0, oldLines: 0, newStart: 3, newLines: 1 }
		]);
	});
});
