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
