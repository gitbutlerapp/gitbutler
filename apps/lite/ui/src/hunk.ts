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
import { Match } from "effect";

type HunkDependencyDiff = HunkDependencies["diffs"][number];

export const hunkContainsHunk = (a: HunkHeader, b: HunkHeader): boolean =>
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
}): Array<string> => {
	const commitIds = new Set<string>();

	for (const [, dependencyHunk, locks] of hunkDependencyDiffs) {
		if (hunk && !hunkContainsHunk(hunk, dependencyHunk)) continue;
		for (const dependency of locks) commitIds.add(dependency.commitId);
	}

	return Array.from(commitIds);
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
};

const lineGroupsFromChangeContent = (
	hunk: Hunk,
	content: ChangeContent,
): Array<HunkLineSelectionGroup> => [
	...(content.deletions > 0
		? [
				{
					side: "deletions",
					start: hunk.deletionStart + content.deletionLineIndex - hunk.deletionLineIndex,
					lines: content.deletions,
				} satisfies HunkLineSelectionGroup,
			]
		: []),
	...(content.additions > 0
		? [
				{
					side: "additions",
					start: hunk.additionStart + content.additionLineIndex - hunk.additionLineIndex,
					lines: content.additions,
				} satisfies HunkLineSelectionGroup,
			]
		: []),
];

export const rangeFromLineGroups = (
	lineGroups: Array<HunkLineSelectionGroup>,
): SelectedLineRange | null => {
	const first = lineGroups[0];
	const last = lineGroups.at(-1);
	if (!first || !last) return null;

	const range: SelectedLineRange = {
		start: first.start,
		side: first.side,
		end: last.start + last.lines - 1,
	};

	if (last.side !== first.side) range.endSide = last.side;

	return range;
};

const contiguousSelectionFromContents = (
	hunk: Hunk,
	contents: Array<ChangeContent>,
): HunkLineSelection | null => {
	const lineGroups: Array<HunkLineSelectionGroup> = [];

	for (const content of contents) {
		for (const group of lineGroupsFromChangeContent(hunk, content)) {
			const previous = lineGroups.at(-1);
			if (previous?.side === group.side && previous.start + previous.lines === group.start)
				previous.lines += group.lines;
			else lineGroups.push(group);
		}
	}

	if (lineGroups.length === 0) return null;

	return {
		hunkHeader: hunkHeaderFromHunk(hunk),
		lineGroups,
	};
};

export function* contiguousSelectionsFromHunk(hunk: Hunk): Generator<HunkLineSelection, void> {
	let contents: Array<ChangeContent> = [];

	for (const content of hunk.hunkContent) {
		if (content.type === "change") {
			contents.push(content);
			continue;
		}

		const selection = contiguousSelectionFromContents(hunk, contents);
		if (selection) yield selection;

		contents = [];
	}

	const selection = contiguousSelectionFromContents(hunk, contents);
	if (selection) yield selection;
}

/** A line as the diff view names it: a number, read on one side of the change. */
type LineQuery = {
	hunks: Array<Hunk>;
	line: number;
	side: SelectionSide;
};

/** The hunk covering the line on that side. Hunks never overlap, so at most one does. */
const hunkByLine = ({ hunks, line, side }: LineQuery): Hunk | null =>
	hunks.find((hunk) => {
		const start = side === "deletions" ? hunk.deletionStart : hunk.additionStart;
		const lines = side === "deletions" ? hunk.deletionCount : hunk.additionCount;
		return line >= start && line < start + lines;
	}) ?? null;

/** The changed group under the line, or nothing when the line is context. */
export const contiguousSelectionByLine = (query: LineQuery): HunkLineSelection | null => {
	const hunk = hunkByLine(query);
	if (!hunk) return null;

	const { line, side } = query;
	return (
		contiguousSelectionsFromHunk(hunk).find((sel) =>
			sel.lineGroups.some(
				(group) => group.side === side && line >= group.start && line < group.start + group.lines,
			),
		) ?? null
	);
};

/**
 * Every changed group in the hunk holding the line. Context lines belong to no group of their own,
 * so a click on one takes the hunk whole.
 */
export const wholeHunkSelectionByLine = (query: LineQuery): HunkLineSelection | null => {
	const hunk = hunkByLine(query);
	if (!hunk) return null;

	const lineGroups = contiguousSelectionsFromHunk(hunk)
		.flatMap((sel) => sel.lineGroups)
		.toArray();
	if (lineGroups.length === 0) return null;

	return {
		hunkHeader: hunkHeaderFromHunk(hunk),
		lineGroups,
	};
};

/** The single changed line under the cursor, rather than its surrounding changed run. */
export const singleLineSelectionByLine = (query: LineQuery): HunkLineSelection | null => {
	const selection = contiguousSelectionByLine(query);
	if (!selection) return null;

	return {
		hunkHeader: selection.hunkHeader,
		lineGroups: [{ side: query.side, start: query.line, lines: 1 }],
	};
};

type DiffStyle = "split" | "unified";

type DiffLinePoint = {
	line: number;
	side: SelectionSide;
	changed: boolean;
	hunk: Hunk;
};

type DiffLineRow = Partial<Record<SelectionSide, DiffLinePoint>>;

type DiffLineIndex = {
	rows: Array<DiffLineRow>;
	indexByLine: Record<SelectionSide, Map<number, number>>;
};

const buildDiffLineIndex = (hunks: Array<Hunk>, diffStyle: DiffStyle): DiffLineIndex => {
	const rows: Array<DiffLineRow> = [];

	for (const hunk of hunks) {
		for (const content of hunk.hunkContent) {
			if (content.type === "context") {
				for (let offset = 0; offset < content.lines; offset++) {
					const additions: DiffLinePoint = {
						line: hunk.additionStart + content.additionLineIndex - hunk.additionLineIndex + offset,
						side: "additions",
						changed: false,
						hunk,
					};
					if (diffStyle === "unified") {
						rows.push({ additions });
						continue;
					}

					rows.push({
						additions,
						deletions: {
							line:
								hunk.deletionStart + content.deletionLineIndex - hunk.deletionLineIndex + offset,
							side: "deletions",
							changed: false,
							hunk,
						},
					});
				}
				continue;
			}

			const deletionStart = hunk.deletionStart + content.deletionLineIndex - hunk.deletionLineIndex;
			const additionStart = hunk.additionStart + content.additionLineIndex - hunk.additionLineIndex;
			if (diffStyle === "unified") {
				for (let offset = 0; offset < content.deletions; offset++) {
					rows.push({
						deletions: {
							line: deletionStart + offset,
							side: "deletions",
							changed: true,
							hunk,
						},
					});
				}
				for (let offset = 0; offset < content.additions; offset++) {
					rows.push({
						additions: {
							line: additionStart + offset,
							side: "additions",
							changed: true,
							hunk,
						},
					});
				}
				continue;
			}

			for (let offset = 0; offset < Math.max(content.deletions, content.additions); offset++) {
				const row: DiffLineRow = {};
				if (offset < content.deletions) {
					row.deletions = {
						line: deletionStart + offset,
						side: "deletions",
						changed: true,
						hunk,
					};
				}
				if (offset < content.additions) {
					row.additions = {
						line: additionStart + offset,
						side: "additions",
						changed: true,
						hunk,
					};
				}
				rows.push(row);
			}
		}
	}

	const indexByLine = {
		deletions: new Map<number, number>(),
		additions: new Map<number, number>(),
	};
	for (const [index, row] of rows.entries()) {
		if (row.deletions) indexByLine.deletions.set(row.deletions.line, index);
		if (row.additions) indexByLine.additions.set(row.additions.line, index);
	}

	return { rows, indexByLine };
};

const lineIndexesByHunks = new WeakMap<Array<Hunk>, Partial<Record<DiffStyle, DiffLineIndex>>>();

/** Lazily build the visual line index once for each stable Pierre hunk array and diff style. */
const getDiffLineIndex = (hunks: Array<Hunk>, diffStyle: DiffStyle): DiffLineIndex => {
	let cached = lineIndexesByHunks.get(hunks);
	if (!cached) lineIndexesByHunks.set(hunks, (cached = {}));
	return (cached[diffStyle] ??= buildDiffLineIndex(hunks, diffStyle));
};

const indexOfPoint = (
	lineIndex: DiffLineIndex,
	line: number,
	side: SelectionSide | undefined,
): number => lineIndex.indexByLine[side ?? "additions"].get(line) ?? -1;

export const selectedLineRangeContainsPoint = ({
	hunks,
	range,
	diffStyle,
	line,
	side,
}: {
	hunks: Array<Hunk>;
	range: SelectedLineRange;
	diffStyle: DiffStyle;
	line: number;
	side: SelectionSide;
}): boolean => {
	const lineIndex = getDiffLineIndex(hunks, diffStyle);
	const start = indexOfPoint(lineIndex, range.start, range.side);
	const end = indexOfPoint(lineIndex, range.end, range.endSide ?? range.side);
	const point = indexOfPoint(lineIndex, line, side);
	if (start === -1 || end === -1 || point === -1) return false;

	return point >= Math.min(start, end) && point <= Math.max(start, end);
};

const changedPointsForRange = (
	lineIndex: DiffLineIndex,
	range: SelectedLineRange,
): Array<DiffLinePoint> => {
	const start = indexOfPoint(lineIndex, range.start, range.side);
	const end = indexOfPoint(lineIndex, range.end, range.endSide ?? range.side);
	if (start === -1 || end === -1) return [];

	const points: Array<DiffLinePoint> = [];
	const first = Math.min(start, end);
	const last = Math.max(start, end);
	for (let index = first; index <= last; index++) {
		const row = lineIndex.rows[index];
		if (row?.deletions?.changed) points.push(row.deletions);
		if (row?.additions?.changed) points.push(row.additions);
	}
	return points;
};

/** Changed lines covered by Pierre's visual range, compacted into one selection per parsed hunk. */
export const lineSelectionsForRange = ({
	hunks,
	range,
	diffStyle,
	granularity = "compact",
}: {
	hunks: Array<Hunk>;
	range: SelectedLineRange;
	diffStyle: DiffStyle;
	granularity?: "compact" | "line";
}): Array<HunkLineSelection> => {
	const lineIndex = getDiffLineIndex(hunks, diffStyle);
	const points = changedPointsForRange(lineIndex, range);
	if (granularity === "line") {
		return points.map((point) => ({
			hunkHeader: hunkHeaderFromHunk(point.hunk),
			lineGroups: [{ side: point.side, start: point.line, lines: 1 }],
		}));
	}

	const selections: Array<HunkLineSelection & { hunk: Hunk }> = [];
	let previousBySide: Partial<Record<SelectionSide, HunkLineSelectionGroup>> = {};
	for (const point of points) {
		let selection = selections.at(-1);
		if (selection?.hunk !== point.hunk) {
			selection = {
				hunk: point.hunk,
				hunkHeader: hunkHeaderFromHunk(point.hunk),
				lineGroups: [],
			};
			selections.push(selection);
			previousBySide = {};
		}

		const previous = previousBySide[point.side];
		if (previous && previous.start + previous.lines === point.line) {
			previous.lines++;
		} else {
			const group = { side: point.side, start: point.line, lines: 1 };
			selection.lineGroups.push(group);
			previousBySide[point.side] = group;
		}
	}

	return selections.map(({ hunk: _, ...selection }) => selection);
};

/** The changed run under the selection edge in the requested direction, or the nearest one beyond it. */
export const hunkSelectionForLineNavigation = <T extends HunkLineSelection>({
	hunks,
	selections,
	range,
	diffStyle,
	offset,
}: {
	hunks: Array<Hunk>;
	selections: Array<T>;
	range: SelectedLineRange;
	diffStyle: DiffStyle;
	offset: -1 | 1;
}): T | null => {
	const lineIndex = getDiffLineIndex(hunks, diffStyle);
	const rangeStart = indexOfPoint(lineIndex, range.start, range.side);
	const rangeEnd = indexOfPoint(lineIndex, range.end, range.endSide ?? range.side);
	if (rangeStart === -1 || rangeEnd === -1) return null;
	const active = offset === 1 ? Math.max(rangeStart, rangeEnd) : Math.min(rangeStart, rangeEnd);

	const positioned = selections
		.values()
		.map((selection) => {
			const selectionRange = rangeFromLineGroups(selection.lineGroups);
			if (!selectionRange) return null;

			const start = indexOfPoint(lineIndex, selectionRange.start, selectionRange.side);
			const end = indexOfPoint(
				lineIndex,
				selectionRange.end,
				selectionRange.endSide ?? selectionRange.side,
			);
			if (start === -1 || end === -1) return null;

			return { selection, start: Math.min(start, end), end: Math.max(start, end) };
		})
		.filter((x) => x != null)
		.toArray();

	const containingIndex = positioned.findIndex(
		({ start, end }) => active >= start && active <= end,
	);
	if (containingIndex !== -1) {
		const containing = positioned[containingIndex];
		if (!containing) return null;
		if (offset === -1 && active === containing.start)
			return positioned[containingIndex - 1]?.selection ?? null;
		if (offset === 1 && active === containing.end)
			return positioned[containingIndex + 1]?.selection ?? null;
		return containing.selection;
	}

	return offset === 1
		? (positioned.find(({ start }) => start > active)?.selection ?? null)
		: (positioned.findLast(({ end }) => end < active)?.selection ?? null);
};

/** Move the active end of Pierre's range by one rendered row. */
export const moveSelectedLineRange = ({
	hunks,
	range,
	diffStyle,
	offset,
	extend,
}: {
	hunks: Array<Hunk>;
	range: SelectedLineRange;
	diffStyle: DiffStyle;
	offset: -1 | 1;
	extend: boolean;
}): SelectedLineRange | null => {
	const lineIndex = getDiffLineIndex(hunks, diffStyle);
	const activeSide = range.endSide ?? range.side ?? "additions";
	const current = indexOfPoint(lineIndex, range.end, activeSide);
	const nextRow = lineIndex.rows[current + offset];
	if (current === -1 || !nextRow) return null;

	const next = nextRow[activeSide] ?? nextRow.additions ?? nextRow.deletions;
	if (!next) return null;

	if (!extend) return { start: next.line, side: next.side, end: next.line };

	return {
		start: range.start,
		...(range.side !== undefined ? { side: range.side } : {}),
		end: next.line,
		...(next.side !== range.side ? { endSide: next.side } : {}),
	};
};

export const diffSpecHunkHeadersForLineSelection = (
	lineSelection: HunkLineSelection,
	action: "commit" | "discard",
): Array<HunkHeader> =>
	lineSelection.lineGroups.map((group): HunkHeader => {
		if (group.side === "deletions") {
			return {
				oldStart: group.start,
				oldLines: group.lines,
				newStart: action === "commit" ? 0 : lineSelection.hunkHeader.newStart,
				newLines: action === "commit" ? 0 : lineSelection.hunkHeader.newLines,
			};
		}

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
