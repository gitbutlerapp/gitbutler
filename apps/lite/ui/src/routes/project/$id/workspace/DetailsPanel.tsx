import uiStyles from "#ui/ui/ui.module.css";
import {
	branchDetailsQueryOptions,
	branchDiffQueryOptions,
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { decodeRefName } from "#ui/api/ref-name.ts";
import { commitTitle } from "#ui/commit.ts";
import {
	formatHunkHeader,
	getDependencyCommitIds,
	getHunkDependencyDiffsByPath,
	type HunkDependencyDiff,
} from "#ui/hunk.ts";
import {
	branchFileParent,
	changesFileParent,
	commitFileParent,
	commitOperand,
	fileOperand,
	hunkOperand,
	type FileParent,
	type Operand,
} from "#ui/operands.ts";
import {
	projectActions,
	selectProjectPanelsState,
	selectProjectSelectionFiles,
} from "#ui/projects/state.ts";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/ui/classes.ts";
import { DependencyIcon } from "#ui/ui/icons.tsx";
import { DiffHunk, HunkHeader, TreeChange, UnifiedPatch } from "@gitbutler/but-sdk";
import { PatchDiff, Virtualizer } from "@pierre/diffs/react";
import { useSuspenseQueries, useSuspenseQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Array, Match, pipe } from "effect";
import { FC, Suspense, useDeferredValue } from "react";
import { Panel, PanelProps } from "react-resizable-panels";
import { DependencyIndicatorButton } from "./DependencyIndicatorButton.tsx";
import styles from "./DetailsPanel.module.css";
import { ShortcutButton } from "#ui/components/ShortcutButton.tsx";
import { workspaceHotkeys } from "#ui/hotkeys.ts";
import { isPanelVisible } from "#ui/panels/state.ts";

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

const HunkDiff: FC<{
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

const hunkKey = (hunk: HunkHeader): string =>
	`${hunk.oldStart}:${hunk.oldLines}:${hunk.newStart}:${hunk.newLines}`;

const Hunk: FC<{
	isResultOfBinaryToTextConversion: boolean;
	projectId: string;
	fileParent: FileParent;
	change: TreeChange;
	hunk: DiffHunk;
	hunkDependencyDiffs?: Array<HunkDependencyDiff>;
}> = ({
	isResultOfBinaryToTextConversion,
	projectId,
	fileParent,
	change,
	hunk,
	hunkDependencyDiffs,
}) => {
	const dependencyCommitIds =
		fileParent._tag === "Changes" && hunkDependencyDiffs
			? getDependencyCommitIds({ hunk, hunkDependencyDiffs })
			: undefined;

	const operand = hunkOperand({
		parent: { parent: fileParent, path: change.path },
		hunkHeader: hunk,
		isResultOfBinaryToTextConversion,
	});

	return (
		<div>
			<OperationSourceC projectId={projectId} selectionScope="files" source={operand}>
				<div className={styles.hunkHeaderRow}>
					{dependencyCommitIds && (
						<DependencyIndicatorButton projectId={projectId} commitIds={dependencyCommitIds}>
							<DependencyIcon />
						</DependencyIndicatorButton>
					)}
					<div className={styles.hunkHeader}>{formatHunkHeader(hunk)}</div>
				</div>
			</OperationSourceC>
			<HunkDiff change={change} diff={hunk.diff} />
		</div>
	);
};

const FileDiff: FC<{
	projectId: string;
	change: TreeChange;
	fileParent: FileParent;
	hunkDependencyDiffs?: Array<HunkDependencyDiff>;
	diff: UnifiedPatch | null;
}> = ({ projectId, change, fileParent, hunkDependencyDiffs, diff }) =>
	Match.value(diff).pipe(
		Match.when(null, () => <div>No diff available for this file.</div>),
		Match.when({ type: "Binary" }, () => <div>Binary file (diff not available).</div>),
		Match.when({ type: "TooLarge" }, ({ subject }) => (
			<div>Diff too large ({subject.sizeInBytes} bytes).</div>
		)),
		Match.when({ type: "Patch" }, (patch) => {
			const { hunks } = patch.subject;
			if (hunks.length === 0) return <p className={styles.emptyFileHunks}>No hunks.</p>;

			return (
				<ul>
					{hunks.map((hunk) => (
						<li key={hunkKey(hunk)}>
							<Hunk
								isResultOfBinaryToTextConversion={patch.subject.isResultOfBinaryToTextConversion}
								projectId={projectId}
								fileParent={fileParent}
								change={change}
								hunk={hunk}
								hunkDependencyDiffs={hunkDependencyDiffs}
							/>
						</li>
					))}
				</ul>
			);
		}),
		Match.exhaustive,
	);

const ChangesFileDiffList: FC<{
	changes: Array<TreeChange>;
	projectId: string;
	fileParent: FileParent;
	hunkDependencyDiffsByPath?: Map<string, Array<HunkDependencyDiff>>;
}> = ({ changes, projectId, fileParent, hunkDependencyDiffsByPath }) => {
	const treeChangeDiffs = useSuspenseQueries({
		queries: changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
	}).map((result) => result.data);
	const changesWithDiffs = pipe(changes, Array.zip(treeChangeDiffs));

	return changesWithDiffs.length === 0 ? (
		<p className={styles.emptyChanges}>No changes.</p>
	) : (
		<ul className={styles.fileDiffsList}>
			{changesWithDiffs.map(([change, diff]) => {
				const source = fileOperand({ parent: fileParent, path: change.path });

				const lastSepIdx = change.path.lastIndexOf("/");
				const mpathInit = lastSepIdx !== -1 ? change.path.slice(0, lastSepIdx + 1) : null;
				const pathLast = lastSepIdx !== -1 ? change.path.slice(lastSepIdx + 1) : change.path;

				return (
					<li key={change.path} className={styles.fileDiff}>
						<OperationSourceC projectId={projectId} selectionScope="files" source={source}>
							<header className={styles.fileHeader}>
								<h4 className={styles.filePath}>
									{mpathInit !== null && <span className={styles.pathInit}>{mpathInit}</span>}
									<span className={styles.pathLast}>{pathLast}</span>
								</h4>
							</header>
						</OperationSourceC>
						<FileDiff
							projectId={projectId}
							change={change}
							fileParent={fileParent}
							hunkDependencyDiffs={hunkDependencyDiffsByPath?.get(change.path)}
							diff={diff}
						/>
					</li>
				);
			})}
		</ul>
	);
};

const ChangesDetails: FC<{
	projectId: string;
	selectedPath?: string;
}> = ({ projectId, selectedPath }) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);
	const selectedChange =
		selectedPath !== undefined
			? worktreeChanges.changes.find((candidate) => candidate.path === selectedPath)
			: undefined;
	const changes = selectedChange ? [selectedChange] : worktreeChanges.changes;

	return (
		<div>
			<header>
				<h3 className={styles.heading}>Changes</h3>
			</header>

			<ChangesFileDiffList
				changes={changes}
				fileParent={changesFileParent}
				hunkDependencyDiffsByPath={hunkDependencyDiffsByPath}
				projectId={projectId}
			/>
		</div>
	);
};

const CommitDetails: FC<{
	projectId: string;
	commitId: string;
	selectedPath?: string | null;
	stackId: string;
}> = ({ projectId, commitId, selectedPath, stackId }) => {
	const { data: commitDetails } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);

	const fileParent = commitFileParent({ stackId, commitId });
	const selectedChange = commitDetails.changes.find((candidate) => candidate.path === selectedPath);
	if (selectedPath !== undefined && !selectedChange) return null;

	const source = commitOperand({ stackId, commitId });

	return (
		<div>
			<OperationSourceC projectId={projectId} selectionScope="outline" source={source}>
				<header>
					<h3 className={styles.heading}>
						{commitTitle(commitDetails.commit.message)}
						{commitDetails.commit.hasConflicts && " ⚠️"}
					</h3>
				</header>
			</OperationSourceC>
			{commitDetails.commit.message.includes("\n") && (
				<p className={styles.commitMessageBody}>
					{commitDetails.commit.message
						.slice(commitDetails.commit.message.indexOf("\n") + 1)
						.trim()}
				</p>
			)}
			<ChangesFileDiffList
				changes={selectedChange ? [selectedChange] : commitDetails.changes}
				fileParent={fileParent}
				projectId={projectId}
			/>
		</div>
	);
};

const BranchDetails: FC<{
	projectId: string;
	branchRef: Array<number>;
	selectedPath?: string;
	stackId: string;
}> = ({ projectId, branchRef, selectedPath, stackId }) => {
	const decodedBranchRef = decodeRefName(branchRef);
	const [{ data: branchDetails }, { data: branchDiff }] = useSuspenseQueries({
		queries: [
			branchDetailsQueryOptions({
				projectId,
				// https://linear.app/gitbutler/issue/GB-1226/unify-branch-identifiers
				branchName: decodedBranchRef.replace(/^refs\/heads\//, ""),
				remote: null,
			}),
			branchDiffQueryOptions({ projectId, branch: decodedBranchRef }),
		],
	});

	const selectedChange =
		selectedPath !== undefined
			? branchDiff.changes.find((candidate) => candidate.path === selectedPath)
			: undefined;
	const changes = selectedChange ? [selectedChange] : branchDiff.changes;

	return (
		<div>
			<header>
				<h3 className={styles.heading}>{branchDetails.name}</h3>
				{branchDetails.prNumber != null && (
					<h4 className={styles.pr}>PR #{branchDetails.prNumber}</h4>
				)}
			</header>

			<ChangesFileDiffList
				changes={changes}
				projectId={projectId}
				fileParent={branchFileParent({ stackId, branchRef })}
			/>
		</div>
	);
};

const Details: FC<{
	projectId: string;
	selection: Operand;
}> = ({ projectId, selection }) =>
	Match.value(selection).pipe(
		Match.tagsExhaustive({
			Stack: () => null,
			Branch: ({ branchRef, stackId }) => (
				<BranchDetails projectId={projectId} branchRef={branchRef} stackId={stackId} />
			),
			ChangesSection: () => <ChangesDetails projectId={projectId} />,
			File: ({ parent, path }) =>
				Match.value(parent).pipe(
					Match.tagsExhaustive({
						Changes: () => <ChangesDetails projectId={projectId} selectedPath={path} />,
						Branch: ({ branchRef, stackId }) => (
							<BranchDetails
								projectId={projectId}
								branchRef={branchRef}
								selectedPath={path}
								stackId={stackId}
							/>
						),
						Commit: ({ commitId, stackId }) => (
							<CommitDetails
								projectId={projectId}
								commitId={commitId}
								stackId={stackId}
								selectedPath={path}
							/>
						),
					}),
				),
			Commit: ({ commitId, stackId }) => (
				<CommitDetails projectId={projectId} commitId={commitId} stackId={stackId} />
			),
			Hunk: () => null,
		}),
	);

export const DetailsPanel: FC<PanelProps> = (panelProps) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const dispatch = useAppDispatch();
	const urgentSelection = useAppSelector((state) => selectProjectSelectionFiles(state, projectId));
	const panelsState = useAppSelector((state) => selectProjectPanelsState(state, projectId));
	const selection = useDeferredValue(urgentSelection);

	const toggleFiles = () => {
		dispatch(projectActions.togglePanel({ projectId, panel: "files" }));
	};

	return (
		<Panel
			{...panelProps}
			className={classes(panelProps.className, styles.panel)}
			style={{ ...panelProps.style, opacity: urgentSelection !== selection ? 0.5 : 1 }}
		>
			<section className={styles.detailsMeta}>
				<ShortcutButton
					className={classes(uiStyles.button, styles.filesBtn)}
					hotkey={workspaceHotkeys.toggleFilesPanel.hotkey}
					hotkeyOptions={{ meta: workspaceHotkeys.toggleFilesPanel.meta }}
					aria-pressed={isPanelVisible(panelsState, "files")}
					onClick={toggleFiles}
				>
					Files
				</ShortcutButton>
			</section>

			<Virtualizer className={styles.detailsVirtualizer}>
				<Suspense fallback={<p className={styles.loading}>Loading details…</p>}>
					<Details projectId={projectId} selection={selection} />
				</Suspense>
			</Virtualizer>
		</Panel>
	);
};
