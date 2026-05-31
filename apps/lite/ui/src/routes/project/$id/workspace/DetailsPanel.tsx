import { SuspenseQuery } from "@suspensive/react-query";
import {
	branchDetailsQueryOptions,
	branchDiffQueryOptions,
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { decodeRefName } from "#ui/api/ref-name.ts";
import { commitBody, commitTitle, shortCommitId } from "#ui/commit.ts";
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
	selectProjectSelectionOutline,
} from "#ui/projects/state.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { Tooltip } from "@base-ui/react";
import { DiffHunk, HunkHeader, TreeChange, UnifiedPatch } from "@gitbutler/but-sdk";
import { PatchDiff, Virtualizer } from "@pierre/diffs/react";
import { useSuspenseQueries } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Array, Match, pipe } from "effect";
import { ComponentProps, FC, Suspense, useDeferredValue } from "react";
import { DependencyIndicatorButton } from "./DependencyIndicatorButton.tsx";
import { FilesPanel } from "./FilesPanel.tsx";
import styles from "./DetailsPanel.module.css";
import { workspaceHotkeys } from "#ui/hotkeys.ts";
import { Panel } from "#ui/panels.ts";

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
							<Icon name="link" />
						</DependencyIndicatorButton>
					)}
					<div className={classes("text-11", "text-monospace", styles.hunkHeader)}>
						{formatHunkHeader(hunk)}
					</div>
				</div>
			</OperationSourceC>

			<PatchDiff
				patch={`${patchHeaderForChange(change, lineEndingForDiff(hunk.diff))}${hunk.diff}`}
				options={{
					diffStyle: "unified",
					themeType: "system",
					disableFileHeader: true,
				}}
			/>
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
			if (hunks.length === 0)
				return <p className={classes("text-13", styles.emptyFileHunks)}>No hunks.</p>;

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
		<p className={classes("text-13", styles.emptyChanges)}>No changes.</p>
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
								<h4 className={classes("text-13", styles.filePath)}>
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

const Header: FC<{
	projectId: string;
	selection: Operand;
}> = ({ projectId, selection }) =>
	Match.value(selection).pipe(
		Match.tagsExhaustive({
			Stack: () => null,
			Branch: ({ branchRef }) => {
				const decodedBranchRef = decodeRefName(branchRef);

				return (
					<SuspenseQuery
						{...branchDetailsQueryOptions({
							projectId,
							// https://linear.app/gitbutler/issue/GB-1226/unify-branch-identifiers
							branchName: decodedBranchRef.replace(/^refs\/heads\//, ""),
							remote: null,
						})}
					>
						{({ data: branchDetails }) => (
							<header className={styles.header}>
								<h3 className={classes("text-14", "text-semibold")}>{branchDetails.name}</h3>
								{branchDetails.prNumber != null && (
									<h4 className={classes("text-13", "text-bold", styles.pr)}>
										PR #{branchDetails.prNumber}
									</h4>
								)}
							</header>
						)}
					</SuspenseQuery>
				);
			},
			ChangesSection: () => (
				<header className={styles.header}>
					<h3 className={classes("text-14", "text-semibold")}>Changes</h3>
				</header>
			),
			File: () => null,
			Commit: ({ commitId, stackId }) => {
				const source = commitOperand({ stackId, commitId });

				return (
					<SuspenseQuery {...commitDetailsWithLineStatsQueryOptions({ projectId, commitId })}>
						{({ data: commitDetails }) => (
							<OperationSourceC projectId={projectId} selectionScope="outline" source={source}>
								<header className={styles.header}>
									<Icon name="commit" />
									<h3 className={classes("text-14", "text-semibold")}>
										{commitTitle(commitDetails.commit.message)}
										{commitDetails.commit.hasConflicts && " ⚠️"}
									</h3>
									<span className={classes("text-13", styles.commitMeta)}>
										#{shortCommitId(commitDetails.commit.id)}
									</span>
								</header>
							</OperationSourceC>
						)}
					</SuspenseQuery>
				);
			},
			Hunk: () => null,
		}),
	);

const FilesToggle: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const dispatch = useAppDispatch();
	const panelsState = useAppSelector((state) => selectProjectPanelsState(state, projectId));

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				className={getButtonClassName({})}
				aria-pressed={panelsState.filesVisible}
				onClick={() => dispatch(projectActions.toggleFilesPanel({ projectId }))}
			>
				Files
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup kbd={workspaceHotkeys.toggleFilesPanel.hotkey} />}>
						{workspaceHotkeys.toggleFilesPanel.meta.name}
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const CommitDetailsContent: FC<{
	projectId: string;
	commitId: string;
}> = ({ projectId, commitId }) => (
	<SuspenseQuery {...commitDetailsWithLineStatsQueryOptions({ projectId, commitId })}>
		{({ data: commitDetails }) => {
			const fmtDate = new Intl.DateTimeFormat(undefined, {
				day: "2-digit",
				month: "2-digit",
				year: "numeric",
				hour: "2-digit",
				minute: "2-digit",
				hour12: false,
			}).format(commitDetails.commit.createdAt);

			const body = commitBody(commitDetails.commit.message);

			return (
				<>
					{body !== undefined && (
						<p className={classes("text-monospace", "text-body", styles.commitMessageBody)}>
							{body}
						</p>
					)}
					<div className={styles.commitDetailsMeta}>
						<img
							src={commitDetails.commit.author.gravatarUrl}
							className={styles.avatar}
							alt="Commit author avatar"
						/>
						<div className={classes("text-13", styles.author)}>
							<span title={commitDetails.commit.author.email}>
								{commitDetails.commit.author.name}
							</span>{" "}
							at {fmtDate}
						</div>
						<div className={classes("text-13", styles.commitMeta)}>
							{shortCommitId(commitDetails.commit.changeId)} (
							{shortCommitId(commitDetails.commit.id)})
						</div>
					</div>
				</>
			);
		}}
	</SuspenseQuery>
);

const DiffContents: FC<{
	projectId: string;
	selection: Operand;
}> = ({ projectId, selection }) =>
	Match.value(selection).pipe(
		Match.tagsExhaustive({
			Stack: () => null,
			Branch: ({ branchRef, stackId }) => (
				<SuspenseQuery {...branchDiffQueryOptions({ projectId, branch: decodeRefName(branchRef) })}>
					{({ data: branchDiff }) => (
						<ChangesFileDiffList
							changes={branchDiff.changes}
							projectId={projectId}
							fileParent={branchFileParent({ stackId, branchRef })}
						/>
					)}
				</SuspenseQuery>
			),
			ChangesSection: () => (
				<SuspenseQuery {...changesInWorktreeQueryOptions(projectId)}>
					{({ data: worktreeChanges }) => (
						<ChangesFileDiffList
							changes={worktreeChanges.changes}
							fileParent={changesFileParent}
							hunkDependencyDiffsByPath={getHunkDependencyDiffsByPath(
								worktreeChanges.dependencies?.diffs ?? [],
							)}
							projectId={projectId}
						/>
					)}
				</SuspenseQuery>
			),

			File: ({ parent, path }) =>
				Match.value(parent).pipe(
					Match.tagsExhaustive({
						Changes: () => (
							<SuspenseQuery {...changesInWorktreeQueryOptions(projectId)}>
								{({ data: worktreeChanges }) => {
									const selectedChange = worktreeChanges.changes.find(
										(candidate) => candidate.path === path,
									);

									return (
										<ChangesFileDiffList
											changes={selectedChange ? [selectedChange] : worktreeChanges.changes}
											fileParent={changesFileParent}
											hunkDependencyDiffsByPath={getHunkDependencyDiffsByPath(
												worktreeChanges.dependencies?.diffs ?? [],
											)}
											projectId={projectId}
										/>
									);
								}}
							</SuspenseQuery>
						),

						Branch: ({ branchRef, stackId }) => (
							<SuspenseQuery
								{...branchDiffQueryOptions({
									projectId,
									branch: decodeRefName(branchRef),
								})}
							>
								{({ data: branchDiff }) => {
									const selectedChange = branchDiff.changes.find(
										(candidate) => candidate.path === path,
									);

									return (
										<ChangesFileDiffList
											changes={selectedChange ? [selectedChange] : branchDiff.changes}
											projectId={projectId}
											fileParent={branchFileParent({ stackId, branchRef })}
										/>
									);
								}}
							</SuspenseQuery>
						),
						Commit: ({ commitId, stackId }) => (
							<SuspenseQuery {...commitDetailsWithLineStatsQueryOptions({ projectId, commitId })}>
								{({ data: commitDetails }) => {
									const fileParent = commitFileParent({ stackId, commitId });
									const selectedChange = commitDetails.changes.find(
										(candidate) => candidate.path === path,
									);
									if (!selectedChange) return null;

									return (
										<ChangesFileDiffList
											changes={[selectedChange]}
											fileParent={fileParent}
											projectId={projectId}
										/>
									);
								}}
							</SuspenseQuery>
						),
					}),
				),
			Commit: ({ commitId, stackId }) => (
				<SuspenseQuery {...commitDetailsWithLineStatsQueryOptions({ projectId, commitId })}>
					{({ data: commitDetails }) => (
						<ChangesFileDiffList
							changes={commitDetails.changes}
							fileParent={commitFileParent({ stackId, commitId })}
							projectId={projectId}
						/>
					)}
				</SuspenseQuery>
			),
			Hunk: () => null,
		}),
	);

export const DetailsPanel: FC<ComponentProps<"div">> = (panelProps) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const panelsState = useAppSelector((state) => selectProjectPanelsState(state, projectId));
	const urgentOutlineSelection = useAppSelector((state) =>
		selectProjectSelectionOutline(state, projectId),
	);
	const outlineSelection = useDeferredValue(urgentOutlineSelection);
	const urgentFilesSelection = useAppSelector((state) =>
		selectProjectSelectionFiles(state, projectId),
	);
	const filesSelection = useDeferredValue(urgentFilesSelection);

	if (outlineSelection._tag === "Stack") return;

	return (
		<div
			{...panelProps}
			className={classes(panelProps.className, styles.panel)}
			style={{ opacity: urgentOutlineSelection !== outlineSelection ? 0.5 : 1 }}
		>
			<div className={styles.headerWrap}>
				<Suspense fallback={<p className="text-13">Loading details…</p>}>
					<Header projectId={projectId} selection={outlineSelection} />

					{outlineSelection._tag === "Commit" && (
						<CommitDetailsContent projectId={projectId} commitId={outlineSelection.commitId} />
					)}
				</Suspense>

				<div>
					<FilesToggle />
				</div>
			</div>

			<div className={classes(styles.panels, panelsState.filesVisible && styles.panelsWithFiles)}>
				{panelsState.filesVisible && (
					<FilesPanel
						id={"files" satisfies Panel}
						data-panel
						tabIndex={0}
						className={styles.filesPanel}
					/>
				)}

				<div
					id={"details" satisfies Panel}
					data-panel
					// oxlint-disable-next-line jsx_a11y/no-noninteractive-tabindex -- Revisit this when we add hunk/line selection.
					tabIndex={0}
					className={styles.detailsContentPanel}
					style={{ opacity: urgentFilesSelection !== filesSelection ? 0.5 : 1 }}
				>
					<Suspense fallback={<p className="text-13">Loading diff…</p>}>
						<Virtualizer className={styles.detailsVirtualizer}>
							<DiffContents projectId={projectId} selection={filesSelection} />
						</Virtualizer>
					</Suspense>
				</div>
			</div>
		</div>
	);
};
