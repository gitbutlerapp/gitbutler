/**
 * @file We have two representations of hunks: `Hunk` from Pierre, and the assorted hunk types from
 * the SDK.
 *
 * When a selection does not exactly match the original SDK hunk it may need to be sent back as
 * multiple synthetic hunk headers: selected additions and deletions are separate headers, ordered
 * by their position in the diff, with context lines omitted.
 */

import type { DiffHunk, HunkDependencies, HunkHeader } from "@gitbutler/but-sdk";
import type { ChangeContent, Hunk, SelectedLineRange, SelectionSide } from "@pierre/diffs";
import { Array } from "effect";

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
	hunk.hunkContent.flatMap((content): HunkLineSelection | [] => {
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

export const diffSpecHunkHeadersForLineSelection = (
	lineSelection: HunkLineSelection,
	action: "commit" | "discard",
): Array<HunkHeader> =>
	lineSelection.lineGroups.map((group): HunkHeader => {
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
