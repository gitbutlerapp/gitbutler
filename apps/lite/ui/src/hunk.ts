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

export type HunkLineSelectionGroup = {
	side: SelectionSide;
	start: number;
	lines: number;
};

export type HunkLineSelection = {
	/** The full parsed hunk containing the selected line groups. */
	hunkHeader: HunkHeader;
	/** Changed-line groups covered by the single visual range, in hunk order. */
	lineGroups: Array<HunkLineSelectionGroup>;
	/** The single CodeView selection range to show for this selection. */
	range: SelectedLineRange;
};

type HunkLineRow = {
	additionLine?: number;
	changedLine?: { side: SelectionSide; line: number };
	deletionLine?: number;
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

const rowMatchesPoint = (row: HunkLineRow, line: number, side: SelectionSide): boolean =>
	side === "deletions" ? row.deletionLine === line : row.additionLine === line;

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
			hunkHeader: hunkHeaderFromHunk(hunk),
			lineGroups,
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
			const containsChangedLine = sel.lineGroups.some(
				(group) => group.side === side && line >= group.start && line < group.start + group.lines,
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
	const startSide = range.side;
	const endSide = range.endSide ?? range.side;
	if (startSide === undefined || endSide === undefined) return null;

	for (const hunk of hunks) {
		const rows = lineRowsFromHunk(hunk);
		const startIndex = rows.findIndex((row) => rowMatchesPoint(row, range.start, startSide));
		const endIndex = rows.findIndex((row) => rowMatchesPoint(row, range.end, endSide));

		if (startIndex === -1 || endIndex === -1) continue;

		const lineGroups = lineGroupsFromRows(
			rows.slice(Math.min(startIndex, endIndex), Math.max(startIndex, endIndex) + 1),
		);
		if (!Array.isNonEmptyArray(lineGroups)) return null;

		return {
			hunkHeader: hunkHeaderFromHunk(hunk),
			lineGroups,
			range,
		};
	}

	return null;
};

const lineGroupsIntersect = (a: HunkLineSelectionGroup, b: HunkLineSelectionGroup): boolean => {
	if (a.side !== b.side) return false;

	const aEnd = a.start + a.lines;
	const bEnd = b.start + b.lines;
	return a.start < bEnd && b.start < aEnd;
};

export const lineSelectionsIntersect = (a: HunkLineSelection, b: HunkLineSelection): boolean =>
	a.lineGroups.some((aGroup) => b.lineGroups.some((bGroup) => lineGroupsIntersect(aGroup, bGroup)));

export const diffSpecHunkHeadersForLineSelection = (
	lineSelection: HunkLineSelection,
	action: "commit" | "discard",
): Array<HunkHeader> =>
	lineSelection.lineGroups.map((group) => {
		if (group.side === "deletions")
			return {
				oldStart: group.start,
				oldLines: group.lines,
				newStart: action === "commit" ? 0 : lineSelection.hunkHeader.newStart,
				newLines: action === "commit" ? 0 : lineSelection.hunkHeader.newLines,
			};

		return {
			oldStart: action === "commit" ? 0 : lineSelection.hunkHeader.oldStart,
			oldLines: action === "commit" ? 0 : lineSelection.hunkHeader.oldLines,
			newStart: group.start,
			newLines: group.lines,
		};
	});

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
