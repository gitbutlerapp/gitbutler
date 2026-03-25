import {
	branchDetailsQueryOptions,
	branchDiffQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
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

const hunkHeaderEquals = (a: HunkHeader, b: HunkHeader): boolean =>
	a.oldStart === b.oldStart &&
	a.oldLines === b.oldLines &&
	a.newStart === b.newStart &&
	a.newLines === b.newLines;

export const formatHunkHeader = (hunk: HunkHeader): string =>
	`-${hunk.oldStart},${hunk.oldLines} +${hunk.newStart},${hunk.newLines}`;

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
}> = ({ diff }) => <pre>{diff.split("\n").slice(1).join("\n")}</pre>;

const hunkKey = (hunk: HunkHeader): string =>
	`${hunk.oldStart}:${hunk.oldLines}:${hunk.newStart}:${hunk.newLines}`;

export type Patch = Extract<UnifiedPatch, { type: "Patch" }>;

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
				<ul>
					{visibleHunks.map((hunk) => (
						<li key={hunkKey(hunk)}>{renderHunk(hunk, patch)}</li>
					))}
				</ul>
			);
		}),
		Match.exhaustive,
	);
};

export const FileButton: FC<
	{
		change: TreeChange;
		toggleSelect: () => void;
	} & ComponentProps<"button">
> = ({ change, toggleSelect, className, ...restProps }) => (
	<button
		{...restProps}
		type="button"
		className={classes(className, styles.fileButton)}
		onClick={toggleSelect}
	>
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

	if (conflictedPaths.length === 0 && data.changes.length === 0) return <div>No file changes.</div>;

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

export const CommitLabel: FC<{
	commit: Commit;
}> = ({ commit }) => (
	<>
		{commit.message === "" ? <>(no message)</> : commit.message.split("\n")[0]}
		{commit.hasConflicts && " ⚠️"}
	</>
);

export const ShowCommit: FC<{
	projectId: string;
	commitId: string;
	renderHunk: (change: TreeChange, hunk: DiffHunk, patch: Patch) => ReactNode;
}> = ({ projectId, commitId, renderHunk }) => {
	const { data } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);
	const firstLineEnd = data.commit.message.indexOf("\n");
	const commitMessageBody =
		firstLineEnd === -1 ? "" : data.commit.message.slice(firstLineEnd + 1).trim();

	return (
		<>
			<h3>
				<CommitLabel commit={data.commit} />
			</h3>
			{commitMessageBody !== "" && <p className={styles.commitMessageBody}>{commitMessageBody}</p>}
			{data.changes.length === 0 ? (
				<div>No file changes.</div>
			) : (
				<ul>
					{data.changes.map((change) => (
						<li key={change.path}>
							<h4>{change.path}</h4>
							<FileDiff
								projectId={projectId}
								change={change}
								renderHunk={(hunk, patch) => renderHunk(change, hunk, patch)}
							/>
						</li>
					))}
				</ul>
			)}
		</>
	);
};

export const ShowBranch: FC<{
	projectId: string;
	branchRef: string;
	branchName: string;
	remote: string | null;
	renderHunk: (change: TreeChange, hunk: DiffHunk, patch: Patch) => ReactNode;
}> = ({ projectId, branchRef, branchName, remote, renderHunk }) => {
	const { data: branchDetails } = useSuspenseQuery(
		branchDetailsQueryOptions({ projectId, branchName, remote }),
	);
	const { data: branchDiff } = useSuspenseQuery(
		branchDiffQueryOptions({ projectId, branch: branchRef }),
	);

	return (
		<>
			<h3>{branchDetails.name}</h3>
			{branchDetails.prNumber != null && <p>PR #{branchDetails.prNumber}</p>}
			{branchDiff.changes.length === 0 ? (
				<div>No file changes.</div>
			) : (
				<ul>
					{branchDiff.changes.map((change) => (
						<li key={change.path}>
							<h4>{change.path}</h4>
							<FileDiff
								projectId={projectId}
								change={change}
								renderHunk={(hunk, patch) => renderHunk(change, hunk, patch)}
							/>
						</li>
					))}
				</ul>
			)}
		</>
	);
};

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
