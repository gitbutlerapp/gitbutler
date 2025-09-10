import { memoize } from '@gitbutler/shared/memoization';
import {
	lineIdKey,
	parseHunk,
	SectionType,
	type LineId,
	type LineLock
} from '@gitbutler/ui/utils/diffParsing';
import type { HunkLocks } from '$lib/dependencies/dependencies';
import type { Prettify } from '@gitbutler/shared/utils/typeUtils';
import 'reflect-metadata';

export type HunkLock = {
	branchId: string;
	commitId: string;
};

export type DiffSpec = {
	/** lossless version of `previous_path` if this was a rename. */
	readonly previousPathBytes: number[] | null;
	/** lossless version of `path`. */
	readonly pathBytes: number[];
	/** The headers of the hunks to use, or empty if all changes are to be used. */
	readonly hunkHeaders: HunkHeader[];
};

export type HunkHeader = {
	/** The 1-based line number at which the previous version of the file started.*/
	readonly oldStart: number;
	/** The non-zero amount of lines included in the previous version of the file.*/
	readonly oldLines: number;
	/** The 1-based line number at which the new version of the file started.*/
	readonly newStart: number;
	/** The non-zero amount of lines included in the new version of the file.*/
	readonly newLines: number;
};

export type HunkAssignmentError = {
	description: string;
};

/**
 * Represents a loose association between a hunk and a stack.
 * A hunk being assigned to a stack means that upon unapplying the stack,
 * the associated hunks will be dumped into a WIP commit and unapplid together with the stack.
 *
 * The hunk assignments are set by the user but also the backednd reconciles those assignments
 * with the workspace hunks when they are updated on disk.
 *
 * Additionally, the hunk dependencies (locking) affects what assignment is possible.
 */
export type HunkAssignment = {
	/**
	 * A stable identifier for the hunk assignment.
	 *   - When a new hunk is first observed (from the uncommitted changes), it is assigned a new id.
	 *   - If a hunk is modified (i.e. it has gained or lost lines), the UUID remains the same.
	 *   - If two or more hunks become merged (due to edits causing the contexts to overlap), the id of the hunk with the most lines is adopted.
	 */
	readonly id: string | null;
	/** The hunk that is being assigned. Together with path_bytes, this identifies the hunk.
	 * If the file is binary, or too large to load, this will be None and in this case the path name is the only identity.
	 */
	readonly hunkHeader: HunkHeader | null;
	/** The file path of the hunk. Used for display. */
	readonly path: string;
	/** The file path of the hunk in bytes. Used to correctly communicate to the backed when creating new assignments */
	readonly pathBytes: number[];
	/** The stack to which the hunk is assigned. If None, the hunk is not assigned to any stack (i.e. it belongs in the unassigned area */
	readonly stackId: string | null;
	/** The line numbers that were added in this hunk. The "after" or "new" line numbers.*/
	readonly lineNumsAdded: number[] | null;
	/** The line numbers that were removed in this hunk. The "before" or "old" line numbers.*/
	readonly lineNumsRemoved: number[] | null;
};

/**
 * A request to update a hunk assignment. If a file has multiple hunks, the UI client needs to send a list of assignment requests with the appropriate hunk headers.
 */
export type HunkAssignmentRequest = {
	/**
	 * The hunk that is being assigned. Together with path_bytes, this identifies the hunk.
	 * If the file is binary, or too large to load, this will be None and in this case the path name is the only identity.
	 * If the file has hunk headers, then header info MUST be provided.
	 */
	hunkHeader: HunkHeader | null;
	/** The file path of the hunk in bytes. */
	pathBytes: number[];
	/**
	 * The stack to which the hunk is assigned. If set to None, the hunk is set as "unassigned".
	 * If a stack id is set, it must be one of the applied stacks.
	 */
	stackId: string | null;
};

/** Indicates that the assignment request was rejected due to locking - the hunk depends on a commit in the stack it is currently in. */
export type AssignmentRejection = {
	/** The request that was rejected. */
	request: HunkAssignmentRequest;
	/** The locks that caused the rejection. */
	locks: HunkLock[];
};

type DeltaLineGroup = {
	type: DeltaLineType;
	lines: LineId[];
};

function getAnchorLineNumber(lineNumber: number, action: 'discard' | 'commit'): number {
	switch (action) {
		case 'discard':
			return lineNumber;
		case 'commit':
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
	action: 'discard' | 'commit'
): HunkHeader {
	const lineCount = lineGroup.lines.length;
	if (lineCount === 0) {
		throw new Error('Line group has no lines');
	}

	const firstLine = lineGroup.lines[0]!;

	switch (lineGroup.type) {
		case 'added': {
			const oldStart = getAnchorLineNumber(parentHunkHeader.oldStart, action);
			const oldLines = getAnchorLineNumber(parentHunkHeader.oldLines, action);
			if (firstLine.newLine === undefined) {
				throw new Error('Line has no new line number');
			}
			const newStart = firstLine.newLine;
			const newLines = lineCount;
			return { oldStart, oldLines, newStart, newLines };
		}
		case 'removed': {
			const newStart = getAnchorLineNumber(parentHunkHeader.newStart, action);
			const newLines = getAnchorLineNumber(parentHunkHeader.newLines, action);
			if (firstLine.oldLine === undefined) {
				throw new Error('Line has no old line number');
			}
			const oldStart = firstLine.oldLine;
			const oldLines = lineCount;
			return { oldStart, oldLines, newStart, newLines };
		}
	}
}

type DeltaLineType = 'added' | 'removed';

function lineType(line: LineId): DeltaLineType | undefined {
	if (line.oldLine === undefined && line.newLine === undefined) {
		throw new Error('Line has no line numbers');
	}
	if (line.oldLine === undefined) {
		return 'added';
	}
	if (line.newLine === undefined) {
		return 'removed';
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
				newLine: line.afterLineNumber
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
		newLines: parsedHunk.newLines
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
	action: 'discard' | 'commit'
): HunkHeader[] {
	if (lineIds.length === 0) {
		return [];
	}

	const [lineGroups, parentHunkHeader] = extractLineGroups(lineIds, diff);

	return lineGroups.map((lineGroup) => lineGroupsToHunkHeader(lineGroup, parentHunkHeader, action));
}

/** A hunk as used in UnifiedDiff. */
export type DiffHunk = Prettify<
	HunkHeader & {
		/**
		 * A unified-diff formatted patch like:
		 *
		 * ```diff
		 * @@ -1,6 +1,8 @@
		 * This is the first line of the original text.
		 * -Line to be removed.
		 * +Line that has been replaced.
		 * This is another line in the file.
		 * +This is a new line added at the end.
		 * ```
		 *
		 * The line separator is the one used in the original file and may be `LF` or `CRLF`.
		 * Note that the file-portion of the header isn't used here.
		 */
		readonly diff: string;
	}
>;

export function isDiffHunk(something: unknown): something is DiffHunk {
	return (
		typeof something === 'object' &&
		something !== null &&
		'oldStart' in something &&
		typeof (something as any).oldStart === 'number' &&
		'oldLines' in something &&
		typeof (something as any).oldLines === 'number' &&
		'newStart' in something &&
		typeof (something as any).newStart === 'number' &&
		'newLines' in something &&
		typeof (something as any).newLines === 'number' &&
		'diff' in something &&
		typeof (something as any).diff === 'string'
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
		throw new Error('Line has no line numbers');
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

	throw new Error('Malformed line ID');
}

/**
 * Get the line locks for a hunk.
 */
export function getLineLocks(
	hunk: DiffHunk,
	locks: HunkLocks[]
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
				newLine: line.afterLineNumber
			};

			const hunkLocks = locksContained.filter((lock) => hunkContainsLine(lock.hunk, lineId));
			if (hunkLocks.length === 0) {
				hunkIsFullyLocked = false;
				continue;
			}

			lineLocks.push({
				...lineId,
				locks: hunkLocks.map((lock) => lock.locks).flat()
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
