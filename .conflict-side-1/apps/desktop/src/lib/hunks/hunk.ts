import {
	lineIdKey,
	parseHunk,
	SectionType,
	type LineId,
	type LineLock
} from '@gitbutler/ui/utils/diffParsing';
import { hashCode } from '@gitbutler/ui/utils/string';
import { Transform, Type } from 'class-transformer';
import type { HunkLocks } from '$lib/dependencies/dependencies';
import type { Prettify } from '@gitbutler/shared/utils/typeUtils';
import 'reflect-metadata';

export class RemoteHunk {
	diff!: string;
	hash?: string;
	oldStart!: number;
	oldLines!: number;
	newStart!: number;
	newLines!: number;
	changeType!: ChangeType;

	get id(): string {
		return hashCode(this.diff);
	}

	get locked() {
		return false;
	}
}
export type ChangeType =
	/// Entry does not exist in old version
	| 'added'
	/// Entry is untracked item in workdir
	| 'untracked'
	/// Entry does not exist in new version
	| 'deleted'
	/// Entry content changed between old and new
	| 'modified';

export class Hunk {
	id!: string;
	diff!: string;
	@Transform((obj) => {
		return new Date(obj.value);
	})
	modifiedAt!: Date;
	filePath!: string;
	hash?: string;
	locked!: boolean;
	@Type(() => HunkLock)
	lockedTo!: HunkLock[];
	/// Indicates that the hunk depends on multiple branches. In this case the hunk cant be moved or comitted.
	poisoned!: boolean;
	changeType!: ChangeType;
	oldStart!: number;
	oldLines!: number;
	newStart!: number;
	newLines!: number;
}

export class HunkLock {
	branchId!: string;
	commitId!: string;
}

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
	const parsedHunk = parseHunk(diff);

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

export function isFileAdditionHunk(hunk: DiffHunk): boolean {
	return hunk.oldStart === 1 && hunk.oldLines === 0;
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
		a.oldStart + a.oldLines - 1 >= b.oldStart + b.oldLines &&
		a.newStart <= b.newStart &&
		a.newStart + a.newLines - 1 >= b.newStart + b.newLines
	);
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
	stackId: string | undefined,
	hunk: DiffHunk,
	locks: HunkLocks[]
): [boolean, LineLock[] | undefined] {
	if (stackId === undefined) {
		return [false, undefined];
	}

	const lineLocks: LineLock[] = [];
	const parsedHunk = parseHunk(hunk.diff);

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

			// Filter out locks to the current stack ID
			const locks = hunkLocks
				.map((lock) => lock.locks)
				.flat()
				.filter((lock) => lock.stackId !== stackId);

			if (locks.length === 0) {
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
