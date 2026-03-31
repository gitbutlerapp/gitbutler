import { memoize } from "@gitbutler/shared/memoization";
import {
	lineIdKey,
	parseHunk,
	SectionType,
	type LineId,
	type LineLock,
} from "@gitbutler/ui/utils/diffParsing";
import type { HunkLocks } from "$lib/hunks/dependencies";
import type { DiffHunk, HunkHeader } from "@gitbutler/but-sdk";

export type HunkAssignmentError = {
	description: string;
};

export function shouldRaiseHunkAssignmentError(
	error: HunkAssignmentError | null,
): error is HunkAssignmentError {
	if (!error) return false;
	if (error.description === "errors.projects.default_target.not_found") return false;
	return true;
}

type DeltaLineGroup = {
	type: DeltaLineType;
	lines: LineId[];
};

function getAnchorLineNumber(lineNumber: number, action: "discard" | "commit"): number {
	switch (action) {
		case "discard":
			return lineNumber;
		case "commit":
			return 0;
	}
}
/**
 * Turn a grouping of lines into a hunk header.
 *
 * This expects the lines to be in order, consecutive and to all be of the same type.
 */
function lineGroupsToHunkHeader(
	lineGroup: DeltaLineGroup,
	parentHunkHeader: HunkHeader,
	action: "discard" | "commit",
): HunkHeader {
	const lineCount = lineGroup.lines.length;
	if (lineCount === 0) {
		throw new Error("Line group has no lines");
	}

	const firstLine = lineGroup.lines[0]!;

	switch (lineGroup.type) {
		case "added": {
			const oldStart = getAnchorLineNumber(parentHunkHeader.oldStart, action);
			const oldLines = getAnchorLineNumber(parentHunkHeader.oldLines, action);
			if (firstLine.newLine === undefined) {
				throw new Error("Line has no new line number");
			}
			const newStart = firstLine.newLine;
			const newLines = lineCount;
			return { oldStart, oldLines, newStart, newLines };
		}
		case "removed": {
			const newStart = getAnchorLineNumber(parentHunkHeader.newStart, action);
			const newLines = getAnchorLineNumber(parentHunkHeader.newLines, action);
			if (firstLine.oldLine === undefined) {
				throw new Error("Line has no old line number");
			}
			const oldStart = firstLine.oldLine;
			const oldLines = lineCount;
			return { oldStart, oldLines, newStart, newLines };
		}
	}
}

type DeltaLineType = "added" | "removed";

function lineType(line: LineId): DeltaLineType | undefined {
	if (line.oldLine === undefined && line.newLine === undefined) {
		throw new Error("Line has no line numbers");
	}
	if (line.oldLine === undefined) {
		return "added";
	}
	if (line.newLine === undefined) {
		return "removed";
	}
	return undefined;
}

const memoizedParseHunk = memoize(parseHunk);

/**
 * Group the selected lines of a diff for the backend.
 *
 * This groups them:
 * - In order based on the provided diff
 * - By type (added, removed, context)
 * - By consecutive line numbers
 */
export function extractLineGroups(lineIds: LineId[], diff: string): [DeltaLineGroup[], HunkHeader] {
	const lineGroups: DeltaLineGroup[] = [];
	let currentGroup: DeltaLineGroup | undefined = undefined;
	const lineKeys = new Set(lineIds.map((lineId) => lineIdKey(lineId)));
	const parsedHunk = memoizedParseHunk(diff);

	for (const section of parsedHunk.contentSections) {
		for (const line of section.lines) {
			const lineId = {
				oldLine: line.beforeLineNumber,
				newLine: line.afterLineNumber,
			};
			const deltaType = lineType(lineId);
			const key = lineIdKey(lineId);

			if (!lineKeys.has(key) || deltaType === undefined) {
				// start a new group
				if (currentGroup !== undefined) {
					currentGroup = undefined;
				}
				continue;
			}

			if (currentGroup === undefined || currentGroup.type !== deltaType) {
				currentGroup = { type: deltaType, lines: [] };
				lineGroups.push(currentGroup);
			}
			currentGroup.lines.push(lineId);
		}
	}
	const parentHunkHeader: HunkHeader = {
		oldStart: parsedHunk.oldStart,
		oldLines: parsedHunk.oldLines,
		newStart: parsedHunk.newStart,
		newLines: parsedHunk.newLines,
	};
	return [lineGroups, parentHunkHeader];
}

export function extractAllGroups(diff: string): [DeltaLineGroup[], HunkHeader] {
	const lineGroups: DeltaLineGroup[] = [];
	let currentGroup: DeltaLineGroup | undefined = undefined;
	const parsedHunk = memoizedParseHunk(diff);

	for (const section of parsedHunk.contentSections) {
		for (const line of section.lines) {
			const lineId = {
				oldLine: line.beforeLineNumber,
				newLine: line.afterLineNumber,
			};
			const deltaType = lineType(lineId);
			if (deltaType === undefined) {
				// start a new group
				if (currentGroup !== undefined) {
					currentGroup = undefined;
				}
				continue;
			}

			if (currentGroup === undefined || currentGroup.type !== deltaType) {
				currentGroup = { type: deltaType, lines: [] };
				lineGroups.push(currentGroup);
			}
			currentGroup.lines.push(lineId);
		}
	}
	const parentHunkHeader: HunkHeader = {
		oldStart: parsedHunk.oldStart,
		oldLines: parsedHunk.oldLines,
		newStart: parsedHunk.newStart,
		newLines: parsedHunk.newLines,
	};
	return [lineGroups, parentHunkHeader];
}

/**
 * Build a list of hunk headers from a list of line IDs.
 *
 * Iterate over the lines of the parsed diff, match them against the given line IDs
 * in order to ensure the correct order of the lines.
 */
export function lineIdsToHunkHeaders(
	lineIds: LineId[],
	diff: string,
	action: "discard" | "commit",
): HunkHeader[] {
	if (lineIds.length === 0) {
		return [];
	}

	const [lineGroups, parentHunkHeader] = extractLineGroups(lineIds, diff);

	return lineGroups.map((lineGroup) => lineGroupsToHunkHeader(lineGroup, parentHunkHeader, action));
}

/**
 * Build a list of hunk headers that cover the entire diff.
 *
 * This is used when the user selects the entire hunk.
 */
export function diffToHunkHeaders(diff: string, action: "discard" | "commit"): HunkHeader[] {
	const [lineGroups, parentHunkHeader] = extractAllGroups(diff);

	return lineGroups.map((lineGroup) => lineGroupsToHunkHeader(lineGroup, parentHunkHeader, action));
}

export function isDiffHunk(something: unknown): something is DiffHunk {
	return (
		typeof something === "object" &&
		something !== null &&
		"oldStart" in something &&
		typeof (something as any).oldStart === "number" &&
		"oldLines" in something &&
		typeof (something as any).oldLines === "number" &&
		"newStart" in something &&
		typeof (something as any).newStart === "number" &&
		"newLines" in something &&
		typeof (something as any).newLines === "number" &&
		"diff" in something &&
		typeof (something as any).diff === "string"
	);
}

/**
 * A patch that if applied to the previous state of the resource would yield the current state.
 * Includes all non-overlapping hunks, including their context lines.
 */
export type Patch = {
	/** All non-overlapping hunks, including their context lines. */
	readonly hunks: DiffHunk[];
	/**
	 * If `true`, a binary to text filter (`textconv` in Git config) was used to obtain the `hunks` in the diff.
	 * This means hunk-based operations must be disabled.
	 */
	readonly isResultOfBinaryToTextConversion: boolean;
	/** The number of lines added in the patch. */
	readonly linesAdded: number;
	/** The number of lines removed in the patch. */
	readonly linesRemoved: number;
};

export function isFileDeletionHunk(hunk: DiffHunk): boolean {
	return hunk.newStart === 1 && hunk.newLines === 0;
}

export function canBePartiallySelected(patch: Patch): boolean {
	if (patch.hunks.length === 0) {
		// Should never happen, but just in case
		return false;
	}

	if (patch.hunks.length === 1 && isFileDeletionHunk(patch.hunks[0]!)) {
		// Only one hunk and it's a file deletion
		return false;
	}

	// TODO: Check if the hunks come from the diff filter
	// See: https://github.com/gitbutlerapp/gitbutler/pull/7893

	return true;
}

export function hunkContainsHunk(a: DiffHunk, b: DiffHunk): boolean {
	return (
		a.oldStart <= b.oldStart &&
		a.oldStart + a.oldLines - 1 >= b.oldStart + b.oldLines - 1 &&
		a.newStart <= b.newStart &&
		a.newStart + a.newLines - 1 >= b.newStart + b.newLines - 1
	);
}

/**
 * Determines whether two hunk headers cover the same positions and ranges.
 *
 * This does not mean that they represent the same diffs or are even for the
 * same file. As such, this should only be used to compare headers within the
 * same file.
 */
export function hunkHeaderEquals(a: HunkHeader, b: HunkHeader): boolean {
	if (a.newLines !== b.newLines) return false;
	if (a.oldLines !== b.oldLines) return false;
	if (a.newStart !== b.newStart) return false;
	if (a.oldStart !== b.oldStart) return false;
	return true;
}

export function hunkContainsLine(hunk: DiffHunk, line: LineId): boolean {
	if (line.oldLine === undefined && line.newLine === undefined) {
		throw new Error("Line has no line numbers");
	}

	if (line.oldLine !== undefined && line.newLine !== undefined) {
		return (
			hunk.oldStart <= line.oldLine &&
			hunk.oldStart + hunk.oldLines - 1 >= line.oldLine &&
			hunk.newStart <= line.newLine &&
			hunk.newStart + hunk.newLines - 1 >= line.newLine
		);
	}

	if (line.oldLine !== undefined) {
		return hunk.oldStart <= line.oldLine && hunk.oldStart + hunk.oldLines - 1 >= line.oldLine;
	}

	if (line.newLine !== undefined) {
		return hunk.newStart <= line.newLine && hunk.newStart + hunk.newLines - 1 >= line.newLine;
	}

	throw new Error("Malformed line ID");
}

/**
 * Get the line locks for a hunk.
 */
export function getLineLocks(
	hunk: DiffHunk,
	locks: HunkLocks[],
): [boolean, LineLock[] | undefined] {
	const lineLocks: LineLock[] = [];
	const parsedHunk = memoizedParseHunk(hunk.diff);

	const locksContained = locks.filter((lock) => hunkContainsHunk(hunk, lock.hunk));

	let hunkIsFullyLocked: boolean = true;

	for (const contentSection of parsedHunk.contentSections) {
		if (contentSection.sectionType === SectionType.Context) continue;

		for (const line of contentSection.lines) {
			const lineId: LineId = {
				oldLine: line.beforeLineNumber,
				newLine: line.afterLineNumber,
			};

			const hunkLocks = locksContained.filter((lock) => hunkContainsLine(lock.hunk, lineId));
			if (hunkLocks.length === 0) {
				hunkIsFullyLocked = false;
				continue;
			}

			lineLocks.push({
				...lineId,
				locks: hunkLocks.map((lock) => lock.locks).flat(),
			});
		}
	}

	return [hunkIsFullyLocked, lineLocks];
}

/**
 * Order hunk headers from the top of a file to the bottom.
 *
 * We expect the headers to have lines selected by having a whole side 0'ed out:
 * ```json
 * {
 *		"oldStart": 0,
 *		"oldLines": 0,
 *		"newStart": 3,
 *		"newLines": 1
 * }
 * ```
 * This is how it'd look to select the added line 3.
 *
 * Sorting them, requires us to compare the non-zeroed sides to each other.
 * This is an example of what a set of sorted headers should look like:
 * ```json
 * {
 *		"oldStart": 0,
 *		"oldLines": 0,
 *		"newStart": 3,
 *		"newLines": 1
 *	},
 *	{
 *		"oldStart": 3,
 *		"oldLines": 1,
 *		"newStart": 0,
 *		"newLines": 0
 *	},
 *	{
 *		"oldStart": 0,
 *		"oldLines": 0,
 *		"newStart": 5,
 *		"newLines": 1
 *	},
 *	{
 *		"oldStart": 5,
 *		"oldLines": 1,
 *		"newStart": 0,
 *		"newLines": 0
 *	}
 * ```
 */
export function orderHeaders(a: HunkHeader, b: HunkHeader): number {
	const startA = a.oldStart || a.newStart;
	const startB = b.oldStart || b.newStart;
	return startA - startB;
}
