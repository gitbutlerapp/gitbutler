/**
 * @file We have two representations of hunks: `Hunk` from Pierre, and the assorted hunk types from
 * the SDK.
 *
 * When a selection does not exactly match the original SDK hunk it may need to be sent back as
 * multiple synthetic hunk headers: selected additions and deletions are separate headers, ordered
 * by their position in the diff, with context lines omitted.
 */

import type { DiffHunk, HunkDependencies, HunkHeader, TreeChange } from "@gitbutler/but-sdk";
import type { ChangeContent, Hunk, SelectedLineRange, SelectionSide } from "@pierre/diffs";
import { Array, Match } from "effect";

type HunkDependencyDiff = HunkDependencies["diffs"][number];

const hunkContainsHunk = (a: HunkHeader, b: HunkHeader): boolean =>
	a.oldStart <= b.oldStart &&
	a.oldStart + a.oldLines - 1 >= b.oldStart + b.oldLines - 1 &&
	a.newStart <= b.newStart &&
	a.newStart + a.newLines - 1 >= b.newStart + b.newLines - 1;

export const getHunkDependencyDiffsByPath = (
	hunkDependencyDiffs: Array<HunkDependencyDiff>,
): Map<string, Array<HunkDependencyDiff>> => {
	const byPath = new Map<string, Array<HunkDependencyDiff>>();

	for (const hunkDependencyDiff of hunkDependencyDiffs) {
		const [path] = hunkDependencyDiff;
		const pathDependencyDiffs = byPath.get(path);
		if (pathDependencyDiffs) pathDependencyDiffs.push(hunkDependencyDiff);
		else byPath.set(path, [hunkDependencyDiff]);
	}

	return byPath;
};

export const getDependencyCommitIds = ({
	hunk,
	hunkDependencyDiffs,
}: {
	hunk?: DiffHunk;
	hunkDependencyDiffs: Array<HunkDependencyDiff>;
}): Array.NonEmptyArray<string> | undefined => {
	const commitIds = new Set<string>();

	for (const [, dependencyHunk, locks] of hunkDependencyDiffs) {
		if (hunk && !hunkContainsHunk(hunk, dependencyHunk)) continue;
		for (const dependency of locks) commitIds.add(dependency.commitId);
	}

	const dependencyCommitIds = globalThis.Array.from(commitIds);
	return Array.isNonEmptyArray(dependencyCommitIds) ? dependencyCommitIds : undefined;
};

const hunkHeaderFromHunk = (hunk: Hunk): HunkHeader => ({
	oldStart: hunk.deletionStart,
	oldLines: hunk.deletionCount,
	newStart: hunk.additionStart,
	newLines: hunk.additionCount,
});

const hunkHeadersEqual = (a: HunkHeader, b: HunkHeader): boolean =>
	a.oldStart === b.oldStart &&
	a.oldLines === b.oldLines &&
	a.newStart === b.newStart &&
	a.newLines === b.newLines;

type HunkLineSelectionGroup = {
	side: SelectionSide;
	start: number;
	lines: number;
};

export type HunkLineSelectionSegment = {
	/** The full parsed hunk containing the selected line groups. */
	hunkHeader: HunkHeader;
	/** Changed-line groups covered by the visual range within this hunk, in hunk order. */
	lineGroups: Array<HunkLineSelectionGroup>;
};

export type HunkLineSelection = {
	/** Per-hunk pieces covered by the single visual range, in file order. */
	segments: Array<HunkLineSelectionSegment>;
	/** The single CodeView selection range to show for this selection. */
	range: SelectedLineRange;
};

type HunkLineRow = {
	additionLine?: number;
	changedLine?: { side: SelectionSide; line: number };
	deletionLine?: number;
};

type HunkLinePosition = {
	hunkIndex: number;
	rowIndex: number;
};

const lineGroupsFromChangeContent = (
	hunk: Hunk,
	content: ChangeContent,
): Array<HunkLineSelectionGroup> => [
	...(content.deletions > 0
		? [
				{
					side: "deletions" as const,
					start: hunk.deletionStart + content.deletionLineIndex - hunk.deletionLineIndex,
					lines: content.deletions,
				},
			]
		: []),
	...(content.additions > 0
		? [
				{
					side: "additions" as const,
					start: hunk.additionStart + content.additionLineIndex - hunk.additionLineIndex,
					lines: content.additions,
				},
			]
		: []),
];

const lineRowsFromHunk = (hunk: Hunk): Array<HunkLineRow> => {
	const rows: Array<HunkLineRow> = [];

	for (const content of hunk.hunkContent) {
		const deletionStart = hunk.deletionStart + content.deletionLineIndex - hunk.deletionLineIndex;
		const additionStart = hunk.additionStart + content.additionLineIndex - hunk.additionLineIndex;

		if (content.type === "context") {
			for (let i = 0; i < content.lines; i++)
				rows.push({
					deletionLine: deletionStart + i,
					additionLine: additionStart + i,
				});
			continue;
		}

		for (let i = 0; i < content.deletions; i++)
			rows.push({
				deletionLine: deletionStart + i,
				changedLine: {
					side: "deletions",
					line: deletionStart + i,
				},
			});

		for (let i = 0; i < content.additions; i++)
			rows.push({
				additionLine: additionStart + i,
				changedLine: {
					side: "additions",
					line: additionStart + i,
				},
			});
	}

	return rows;
};

const rowMatchesPoint = (row: HunkLineRow, line: number, side?: SelectionSide): boolean =>
	side === undefined
		? row.deletionLine === line || row.additionLine === line
		: side === "deletions"
			? row.deletionLine === line
			: row.additionLine === line;

const compareLinePositions = (a: HunkLinePosition, b: HunkLinePosition): number =>
	a.hunkIndex === b.hunkIndex ? a.rowIndex - b.rowIndex : a.hunkIndex - b.hunkIndex;

const orderedLinePositions = (
	a: HunkLinePosition,
	b: HunkLinePosition,
): [HunkLinePosition, HunkLinePosition] => (compareLinePositions(a, b) <= 0 ? [a, b] : [b, a]);

const findLinePosition = ({
	hunks,
	line,
	side,
}: {
	hunks: Array<Hunk>;
	line: number;
	side?: SelectionSide;
}): HunkLinePosition | null => {
	for (const [hunkIndex, hunk] of hunks.entries()) {
		const rowIndex = lineRowsFromHunk(hunk).findIndex((row) => rowMatchesPoint(row, line, side));
		if (rowIndex !== -1) return { hunkIndex, rowIndex };
	}

	return null;
};

const lineGroupsFromRows = (rows: Array<HunkLineRow>): Array<HunkLineSelectionGroup> => {
	const lineGroups: Array<HunkLineSelectionGroup> = [];

	for (const row of rows) {
		if (!row.changedLine) continue;

		const last = lineGroups.at(-1);
		if (
			last &&
			last.side === row.changedLine.side &&
			last.start + last.lines === row.changedLine.line
		) {
			last.lines++;
			continue;
		}

		lineGroups.push({
			side: row.changedLine.side,
			start: row.changedLine.line,
			lines: 1,
		});
	}

	return lineGroups;
};

const rangeFromLineGroups = (
	lineGroups: Array.NonEmptyArray<HunkLineSelectionGroup>,
): SelectedLineRange => {
	const first = Array.headNonEmpty(lineGroups);
	const last = Array.lastNonEmpty(lineGroups);
	const range: SelectedLineRange = {
		start: first.start,
		side: first.side,
		end: last.start + last.lines - 1,
	};

	if (last.side !== first.side) range.endSide = last.side;

	return range;
};

export const contiguousSelectionsFromHunk = (hunk: Hunk): Array<HunkLineSelection> =>
	hunk.hunkContent.flatMap((content) => {
		if (content.type !== "change") return [];

		const lineGroups = lineGroupsFromChangeContent(hunk, content);
		if (!Array.isNonEmptyArray(lineGroups)) return [];

		return {
			segments: [
				{
					hunkHeader: hunkHeaderFromHunk(hunk),
					lineGroups,
				},
			],
			range: rangeFromLineGroups(lineGroups),
		};
	});

export const contiguousSelectionByLine = ({
	hunks,
	line,
	side,
}: {
	hunks: Array<Hunk>;
	line: number;
	side: SelectionSide;
}): HunkLineSelection | null => {
	for (const hunk of hunks)
		for (const sel of contiguousSelectionsFromHunk(hunk)) {
			const containsChangedLine = sel.segments.some((segment) =>
				segment.lineGroups.some(
					(group) => group.side === side && line >= group.start && line < group.start + group.lines,
				),
			);
			if (containsChangedLine) return sel;
		}

	return null;
};

export const lineSelectionFromRange = ({
	hunks,
	range,
}: {
	hunks: Array<Hunk>;
	range: SelectedLineRange;
}): HunkLineSelection | null => {
	const startPosition = findLinePosition({ hunks, line: range.start, side: range.side });
	const endPosition = findLinePosition({
		hunks,
		line: range.end,
		side: range.endSide ?? range.side,
	});
	if (startPosition === null || endPosition === null) return null;

	const [firstPosition, lastPosition] = orderedLinePositions(startPosition, endPosition);
	const segments: Array<HunkLineSelectionSegment> = [];

	for (const [hunkIndex, hunk] of hunks.entries()) {
		if (hunkIndex < firstPosition.hunkIndex || hunkIndex > lastPosition.hunkIndex) continue;

		const rows = lineRowsFromHunk(hunk);
		if (rows.length === 0) continue;

		const startIndex = hunkIndex === firstPosition.hunkIndex ? firstPosition.rowIndex : 0;
		const endIndex = hunkIndex === lastPosition.hunkIndex ? lastPosition.rowIndex : rows.length - 1;
		const lineGroups = lineGroupsFromRows(
			rows.slice(Math.min(startIndex, endIndex), Math.max(startIndex, endIndex) + 1),
		);

		segments.push({
			hunkHeader: hunkHeaderFromHunk(hunk),
			lineGroups,
		});
	}

	return { segments, range };
};

const lineGroupsIntersect = (a: HunkLineSelectionGroup, b: HunkLineSelectionGroup): boolean => {
	if (a.side !== b.side) return false;

	const aEnd = a.start + a.lines;
	const bEnd = b.start + b.lines;
	return a.start < bEnd && b.start < aEnd;
};

export const lineSelectionsIntersect = (a: HunkLineSelection, b: HunkLineSelection): boolean =>
	a.segments.some((aSegment) =>
		b.segments.some(
			(bSegment) =>
				hunkHeadersEqual(aSegment.hunkHeader, bSegment.hunkHeader) &&
				aSegment.lineGroups.some((aGroup) =>
					bSegment.lineGroups.some((bGroup) => lineGroupsIntersect(aGroup, bGroup)),
				),
		),
	);

const lineSelectionPosition = ({
	hunks,
	selection,
}: {
	hunks: Array<Hunk>;
	selection: HunkLineSelection;
}): { start: HunkLinePosition; end: HunkLinePosition } | null => {
	const startPosition = findLinePosition({
		hunks,
		line: selection.range.start,
		side: selection.range.side,
	});
	const endPosition = findLinePosition({
		hunks,
		line: selection.range.end,
		side: selection.range.endSide ?? selection.range.side,
	});
	if (startPosition === null || endPosition === null) return null;

	const [start, end] = orderedLinePositions(startPosition, endPosition);
	return { start, end };
};

export const compareLineSelections = ({
	hunks,
	a,
	b,
}: {
	hunks: Array<Hunk>;
	a: HunkLineSelection;
	b: HunkLineSelection;
}): number | null => {
	const aPosition = lineSelectionPosition({ hunks, selection: a });
	const bPosition = lineSelectionPosition({ hunks, selection: b });
	if (aPosition === null || bPosition === null) return null;

	if (compareLinePositions(aPosition.end, bPosition.start) < 0) return -1;
	if (compareLinePositions(aPosition.start, bPosition.end) > 0) return 1;
	return 0;
};

export const diffSpecHunkHeadersForLineSelection = (
	lineSelection: HunkLineSelection,
	action: "commit" | "discard",
): Array<HunkHeader> =>
	lineSelection.segments.flatMap((segment) =>
		segment.lineGroups.map((group) => {
			if (group.side === "deletions")
				return {
					oldStart: group.start,
					oldLines: group.lines,
					newStart: action === "commit" ? 0 : segment.hunkHeader.newStart,
					newLines: action === "commit" ? 0 : segment.hunkHeader.newLines,
				};

			return {
				oldStart: action === "commit" ? 0 : segment.hunkHeader.oldStart,
				oldLines: action === "commit" ? 0 : segment.hunkHeader.oldLines,
				newStart: group.start,
				newLines: group.lines,
			};
		}),
	);

const lineEndingForDiff = (diff: string): string => (diff.includes("\r\n") ? "\r\n" : "\n");

// This is built with Pierre in mind. It's currently incomplete.
const patchHeaderForChange = (change: TreeChange, lineEnding: string): string =>
	Match.value(change.status).pipe(
		Match.when(
			{ type: "Addition" },
			() =>
				[
					`diff --git a/${change.path} b/${change.path}`,
					"new file mode 100644",
					"--- /dev/null",
					`+++ b/${change.path}`,
				].join(lineEnding) + lineEnding,
		),

		Match.when(
			{ type: "Deletion" },
			() =>
				[
					`diff --git a/${change.path} b/${change.path}`,
					"deleted file mode 100644",
					`--- a/${change.path}`,
					"+++ /dev/null",
				].join(lineEnding) + lineEnding,
		),

		Match.when(
			{ type: "Modification" },
			() =>
				[
					`diff --git a/${change.path} b/${change.path}`,
					`--- a/${change.path}`,
					`+++ b/${change.path}`,
				].join(lineEnding) + lineEnding,
		),

		Match.when(
			{ type: "Rename" },
			({ subject }) =>
				[
					`diff --git a/${subject.previousPath} b/${change.path}`,
					"similarity index 99%",
					`rename from ${subject.previousPath}`,
					`rename to ${change.path}`,
					`--- a/${subject.previousPath}`,
					`+++ b/${change.path}`,
				].join(lineEnding) + lineEnding,
		),

		Match.exhaustive,
	);

/** Combine multiple hunks for one file into a single patch, consumable by Pierre. */
export const synthesizeFilePatch = (change: TreeChange, hunks: Array<DiffHunk>): string => {
	const lineEnding = lineEndingForDiff(hunks[0]?.diff ?? "");
	const header = patchHeaderForChange(change, lineEnding);
	return header + hunks.map((hunk) => hunk.diff).join("");
};
