import { PatchDiff } from "@pierre/diffs/react";
import { commitDetailsWithLineStatsQueryOptions } from "#ui/api/queries.ts";
import { classes } from "#ui/classes.ts";
import {
	Commit,
	DiffHunk,
	HunkAssignment,
	HunkHeader,
	TreeChange,
	UnifiedPatch,
} from "@gitbutler/but-sdk";
import { useSuspenseQuery } from "@tanstack/react-query";
import { Match } from "effect";
import { ComponentProps, FC, ReactNode } from "react";
import styles from "./-shared.module.css";

// https://linear.app/gitbutler/issue/GB-1161/refsbranches-should-use-bytes-instead-of-strings
export const decodeRefName = (fullNameBytes: Array<number>): string =>
	new TextDecoder().decode(Uint8Array.from(fullNameBytes));

export const isTypingTarget = (target: EventTarget | null) => {
	if (!(target instanceof HTMLElement)) return false;
	return (
		target.isContentEditable ||
		target instanceof HTMLInputElement ||
		target instanceof HTMLTextAreaElement
	);
};

/** @public */
export const assert = <T,>(t: T | null | undefined): T => {
	if (t == null) throw new Error("Expected value to be non-null and defined");
	return t;
};

export const getRelative = <T,>(items: Array<T>, index: number, offset: -1 | 1): T | null => {
	const itemCount = items.length;
	if (itemCount === 0) return null;
	return items[(index + offset + itemCount) % itemCount] ?? null;
};
export const formatHunkHeader = (hunk: HunkHeader): string =>
	`-${hunk.oldStart},${hunk.oldLines} +${hunk.newStart},${hunk.newLines}`;

export const shortCommitId = (commitId: string): string => commitId.slice(0, 7);

const hunkHeaderEquals = (a: HunkHeader, b: HunkHeader): boolean =>
	a.oldStart === b.oldStart &&
	a.oldLines === b.oldLines &&
	a.newStart === b.newStart &&
	a.newLines === b.newLines;

export const assignedHunks = (
	hunks: Array<DiffHunk>,
	assignments: Array<HunkAssignment>,
): Array<DiffHunk> => {
	if (assignments.length === 0) return [];
	if (assignments.some((assignment) => assignment.hunkHeader == null)) return hunks;

	return hunks.filter((hunk) =>
		assignments.some(
			(assignment) =>
				assignment.hunkHeader != null && hunkHeaderEquals(hunk, assignment.hunkHeader),
		),
	);
};

const lineEndingForDiff = (diff: string): string => (diff.includes("\r\n") ? "\r\n" : "\n");

const patchHeaderForChange = (change: TreeChange, lineEnding: string): string =>
	Match.value(change.status).pipe(
		Match.when(
			{ type: "Addition" },
			() => `--- /dev/null${lineEnding}+++ ${change.path}${lineEnding}`,
		),
		Match.when(
			{ type: "Deletion" },
			() => `--- ${change.path}${lineEnding}+++ /dev/null${lineEnding}`,
		),
		Match.when(
			{ type: "Modification" },
			() => `--- ${change.path}${lineEnding}+++ ${change.path}${lineEnding}`,
		),
		Match.when(
			{ type: "Rename" },
			({ subject }) => `--- ${subject.previousPath}${lineEnding}+++ ${change.path}${lineEnding}`,
		),
		Match.exhaustive,
	);

export const HunkDiff: FC<{
	change: TreeChange;
	diff: string;
}> = ({ change, diff }) => (
	<PatchDiff
		patch={`${patchHeaderForChange(change, lineEndingForDiff(diff))}${diff}`}
		options={{
			diffStyle: "unified",
			themeType: "system",
			disableFileHeader: true,
		}}
	/>
);

export const hunkKey = (hunk: HunkHeader): string =>
	`${hunk.oldStart}:${hunk.oldLines}:${hunk.newStart}:${hunk.newLines}`;

export type Patch = Extract<UnifiedPatch, { type: "Patch" }>;

export const FileButton: FC<
	{
		change: TreeChange;
	} & ComponentProps<"button">
> = ({ change, className, ...restProps }) => (
	<button {...restProps} type="button" className={classes(className, styles.fileButton)}>
		{Match.value(change.status).pipe(
			Match.when({ type: "Addition" }, () => "A"),
			Match.when({ type: "Deletion" }, () => "D"),
			Match.when({ type: "Modification" }, () => "M"),
			Match.when({ type: "Rename" }, () => "R"),
			Match.exhaustive,
		)}{" "}
		{change.path}
	</button>
);

export const CommitDetails: FC<{
	projectId: string;
	commitId: string;
	renderFile: (change: TreeChange) => ReactNode;
}> = ({ projectId, commitId, renderFile }) => {
	const { data } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);

	const conflictedPaths = data.conflictEntries
		? Array.from(
				new Set([
					...data.conflictEntries.ancestorEntries,
					...data.conflictEntries.ourEntries,
					...data.conflictEntries.theirEntries,
				]),
			).sort((a, b) => a.localeCompare(b))
		: [];

	if (conflictedPaths.length === 0 && data.changes.length === 0)
		return <div className={styles.itemEmpty}>No file changes.</div>;

	return (
		<>
			{conflictedPaths.length > 0 && (
				<div>
					<div>Conflicts:</div>
					<ul>
						{conflictedPaths.map((path) => (
							<li key={path}>{path}</li>
						))}
					</ul>
				</div>
			)}

			{data.changes.length > 0 && (
				<ul>
					{data.changes.map((file) => (
						<li key={file.path}>{renderFile(file)}</li>
					))}
				</ul>
			)}
		</>
	);
};

export const commitTitle = (message: string): string => {
	const _title = message.trim().split("\n")[0];
	const title = _title === "" ? undefined : _title;
	return title ?? "(no message)";
};

export const CommitLabel: FC<{
	commit: Commit;
}> = ({ commit }) => (
	<>
		{commitTitle(commit.message)}
		{commit.hasConflicts && " ⚠️"}
	</>
);

export const CommitsList: FC<{
	commits: Array<Commit>;
	children: (commit: Commit, index: number) => ReactNode;
}> = ({ commits, children }) => {
	if (commits.length === 0) return <div>No commits.</div>;

	return (
		<ul>
			{commits.map((commit, index) => (
				<li key={commit.id}>{children(commit, index)}</li>
			))}
		</ul>
	);
};
