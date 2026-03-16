import {
	Commit,
	DiffHunk,
	HunkAssignment,
	HunkHeader,
	TreeChange,
	UnifiedPatch,
} from "@gitbutler/but-sdk";
import { Match } from "effect";
import { useSuspenseQuery } from "@tanstack/react-query";
import { ComponentProps, FC, ReactNode } from "react";
import styles from "./project-shared.module.css";
import {
	commitDetailsWithLineStatsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/queries.ts";

/**
 * @example
 * classes("foo", undefined, "bar", "", "baz") === "foo bar baz"
 */
const classes = (...xs: Array<string | null | undefined | false>): string =>
	// oxlint-disable-next-line typescript/strict-boolean-expressions
	xs.reduce((acc: string, x) => (x ? (acc ? `${acc} ${x}` : x) : acc), "");

type Patch = Extract<UnifiedPatch, { type: "Patch" }>;

const hunkHeaderEquals = (a: HunkHeader, b: HunkHeader): boolean =>
	a.oldStart === b.oldStart &&
	a.oldLines === b.oldLines &&
	a.newStart === b.newStart &&
	a.newLines === b.newLines;

const assignedHunks = (
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

export const HunkDiff: FC<{
	diff: string;
}> = ({ diff }) => <pre className={styles.hunkDiff}>{diff}</pre>;

export const hunkKey = (hunk: HunkHeader): string =>
	`${hunk.oldStart}:${hunk.oldLines}:${hunk.newStart}:${hunk.newLines}`;

export const FileDiff: FC<{
	projectId: string;
	change: TreeChange;
	assignments?: Array<HunkAssignment>;
	renderHunk: (hunk: DiffHunk, patch: Patch) => ReactNode;
}> = ({ projectId, change, assignments, renderHunk }) => {
	const { data } = useSuspenseQuery(treeChangeDiffsQueryOptions({ projectId, change }));

	return Match.value(data).pipe(
		Match.when(null, () => <div>No diff available for this file.</div>),
		Match.when({ type: "Binary" }, () => <div>Binary file (diff not available).</div>),
		Match.when({ type: "TooLarge" }, ({ subject }) => (
			<div>Diff too large ({subject.sizeInBytes} bytes).</div>
		)),
		Match.when({ type: "Patch" }, (patch) => {
			const visibleHunks = assignments
				? assignedHunks(patch.subject.hunks, assignments)
				: patch.subject.hunks;

			if (visibleHunks.length === 0) return <div>No hunks.</div>;

			return (
				<ul className={styles.hunks}>{visibleHunks.map((hunk) => renderHunk(hunk, patch))}</ul>
			);
		}),
		Match.exhaustive,
	);
};

export const FileButton: FC<
	{
		change: TreeChange;
		isSelected: boolean;
		toggleSelect: () => void;
	} & ComponentProps<"button">
> = ({ change, isSelected, toggleSelect, className, ...restProps }) => (
	<button
		{...restProps}
		type="button"
		className={classes(className, styles.fileButton, isSelected && styles.selected)}
		onClick={toggleSelect}
	>
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

	if (conflictedPaths.length === 0 && data.changes.length === 0) return <div>No file changes.</div>;

	return (
		<>
			{conflictedPaths.length > 0 && (
				<div>
					<div>Conflicts:</div>
					<ul className={styles.fileList}>
						{conflictedPaths.map((path) => (
							<li key={path}>{path}</li>
						))}
					</ul>
				</div>
			)}

			{data.changes.length > 0 && (
				<ul className={styles.fileList}>{data.changes.map(renderFile)}</ul>
			)}
		</>
	);
};

export const CommitButton: FC<
	{
		commit: Commit;
		isSelected: boolean;
		isAnyFileSelected: boolean;
		isHighlighted: boolean;
		toggleSelect: () => void;
	} & ComponentProps<"button">
> = ({
	commit,
	isSelected,
	isAnyFileSelected,
	isHighlighted,
	toggleSelect,
	className,
	...restProps
}) => (
	<button
		{...restProps}
		type="button"
		className={classes(
			className,
			styles.commitButton,
			isSelected ? styles.selected : isAnyFileSelected ? styles.selectedWithin : undefined,
		)}
		onClick={toggleSelect}
		style={{
			...(isHighlighted && { backgroundColor: "yellow" }),
		}}
	>
		{commit.message === "" ? <>(no message)</> : commit.message.split("\n")[0]}
		{commit.hasConflicts && " ⚠️"}
	</button>
);

export const CommitsList: FC<{
	commits: Array<Commit>;
	children: (commit: Commit, index: number) => ReactNode;
}> = ({ commits, children }) => {
	if (commits.length === 0) return <div>No commits.</div>;

	return (
		<ul className={styles.commitsList}>
			{commits.map((commit, index) => children(commit, index))}
		</ul>
	);
};
