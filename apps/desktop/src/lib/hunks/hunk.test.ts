import {
	lineIdsToHunkHeaders,
	extractLineGroups,
	hunkContainsHunk,
	hunkContainsLine,
	getLineLocks
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
		const [fullyLocked, lineLocks] = getLineLocks('stack2', hunk, locks);
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
		const [fullyLocked, lineLocks] = getLineLocks('stack1', hunk, noLocks);
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
		const [fullyLocked, lineLocks] = getLineLocks('stack2', partialHunk, partialLocks);
		expect(fullyLocked).toBe(false);
		expect(lineLocks).toEqual([
			{ oldLine: 3, newLine: undefined, locks: [{ stackId: 'stack1', commitId: 'commit1' }] },
			{ oldLine: undefined, newLine: 3, locks: [{ stackId: 'stack1', commitId: 'commit1' }] }
		]);
	});
});
