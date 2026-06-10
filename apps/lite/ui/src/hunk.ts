/**
 * @file We have two representations of hunks: `Hunk` from Pierre, and the assorted hunk types from
 * the SDK.
 */

import type { DiffHunk, HunkDependencies, HunkHeader, TreeChange } from "@gitbutler/but-sdk";
import type { Hunk, SelectedLineRange, SelectionSide } from "@pierre/diffs";
import { Array, Match } from "effect";

type HunkDependencyDiff = HunkDependencies["diffs"][number];

export const formatHunkHeader = (hunk: HunkHeader): string =>
	`-${hunk.oldStart},${hunk.oldLines} +${hunk.newStart},${hunk.newLines}`;

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

export const hunkHeaderFromHunk = (hunk: Hunk): HunkHeader => ({
	oldStart: hunk.deletionStart,
	oldLines: hunk.deletionCount,
	newStart: hunk.additionStart,
	newLines: hunk.additionCount,
});

export const hunkContainsLine = (hunk: Hunk, line: number, side: SelectionSide): boolean => {
	const start = side === "deletions" ? hunk.deletionStart : hunk.additionStart;
	const count = side === "deletions" ? hunk.deletionCount : hunk.additionCount;

	return line >= start && line < start + count;
};

export const selectEntireHunk = (hunk: Hunk): SelectedLineRange => {
	if (hunk.deletionCount > 0 && hunk.additionCount > 0) {
		const lastContent = hunk.hunkContent.at(-1);
		const startsWithAddition =
			hunk.hunkContent[0]?.type === "change" && hunk.hunkContent[0].deletions === 0;
		const endsWithDeletion = lastContent?.type === "change" && lastContent.additions === 0;

		return {
			start: startsWithAddition ? hunk.additionStart : hunk.deletionStart,
			side: startsWithAddition ? "additions" : "deletions",
			end: endsWithDeletion
				? hunk.deletionStart + hunk.deletionCount - 1
				: hunk.additionStart + hunk.additionCount - 1,
			endSide: endsWithDeletion ? "deletions" : "additions",
		};
	}

	if (hunk.deletionCount > 0)
		return {
			start: hunk.deletionStart,
			side: "deletions",
			end: hunk.deletionStart + hunk.deletionCount - 1,
		};

	return {
		start: hunk.additionStart,
		side: "additions",
		end: hunk.additionStart + hunk.additionCount - 1,
	};
};

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
	return [header, ...hunks.map((hunk) => hunk.diff)].join(lineEnding);
};
