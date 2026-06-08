import uiStyles from "#ui/components/ui.module.css";
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
	branchFileParent,
	branchOperand,
	changesFileParent,
	changesSectionOperand,
	commitFileParent,
	commitOperand,
	fileOperand,
	operandIdentityKey,
	type CommitOperand,
	type FileOperand,
	type Operand,
} from "#ui/operands.ts";
import { projectActions, selectProjectFilesVisible } from "#ui/projects/state.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { Tooltip } from "@base-ui/react";
import type {
	CommitDetails,
	DiffHunk,
	TreeChange,
	TreeChanges,
	UnifiedPatch,
	WorktreeChanges,
} from "@gitbutler/but-sdk";
import {
	type CodeViewDiffItem,
	type CodeView as CodeViewClass,
	parsePatchFiles,
} from "@pierre/diffs";
import { CodeView, type CodeViewHandle } from "@pierre/diffs/react";
import { useSuspenseQueries } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Array, Hash, Match } from "effect";
import { ComponentProps, FC, type RefObject, Suspense, useDeferredValue, useRef } from "react";
import styles from "./Details.module.css";
import { workspaceHotkeys } from "#ui/hotkeys.ts";
import { SelectionScope } from "#ui/selection-scopes.ts";
import {
	FilesTree,
	changeFileTreeItem,
	conflictFileTreeItem,
	type FileTreeItem,
} from "#ui/routes/project/$id/workspace/FilesTree.tsx";
import { getDependencyCommitIds, getHunkDependencyDiffsByPath } from "#ui/hunk.ts";
import { buildNavigationIndex } from "#ui/workspace/navigation-index.ts";

const lineEndingForDiff = (diff: string): string => (diff.includes("\r\n") ? "\r\n" : "\n");

const codeViewItemId = ({ changesetKey, path }: { changesetKey: string; path: string }): string =>
	`${changesetKey}:${path}`;

const codeViewItemIdPath = ({ changesetKey, id }: { changesetKey: string; id: string }): string =>
	id.slice(changesetKey.length + 1);

const getScrollTargetId = ({
	changesetKey,
	selection,
}: {
	changesetKey: string;
	selection: FileOperand | null;
}): string | null => (selection ? codeViewItemId({ changesetKey, path: selection.path }) : null);

const getChangesetKey = (selection: Operand): string =>
	Match.value(selection).pipe(
		Match.tags({
			Branch: ({ branchRef }) => decodeRefName(branchRef),
			ChangesSection: () => "changes",
			Commit: ({ commitId }) => commitId,
		}),
		Match.orElseAbsurd,
	);

const getCommitFileTreeItems = ({
	commit,
	commitDetails,
}: {
	commit: CommitOperand;
	commitDetails: CommitDetails;
}): Array<FileTreeItem> => {
	const conflictedPaths = commitDetails.conflictEntries
		? globalThis.Array.from(
				new Set([
					...commitDetails.conflictEntries.ancestorEntries,
					...commitDetails.conflictEntries.ourEntries,
					...commitDetails.conflictEntries.theirEntries,
				]),
			).toSorted((a, b) => a.localeCompare(b))
		: [];
	const conflictedPathSet = new Set(conflictedPaths);

	return [
		...conflictedPaths.map((path) =>
			conflictFileTreeItem({
				operand: {
					parent: commitFileParent(commit),
					path,
				},
				path,
			}),
		),
		...commitDetails.changes
			.filter((change) => !conflictedPathSet.has(change.path))
			.map((change) =>
				changeFileTreeItem({
					change,
					operand: {
						parent: commitFileParent(commit),
						path: change.path,
					},
				}),
			),
	];
};

const getChangesFileTreeItems = (worktreeChanges: WorktreeChanges): Array<FileTreeItem> => {
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);

	return worktreeChanges.changes.map((change) => {
		const hunkDependencyDiffs = hunkDependencyDiffsByPath.get(change.path);
		const dependencyCommitIds = hunkDependencyDiffs
			? getDependencyCommitIds({ hunkDependencyDiffs })
			: undefined;

		return changeFileTreeItem({
			change,
			dependencyCommitIds,
			operand: {
				parent: changesFileParent,
				path: change.path,
			},
		});
	});
};

const getBranchFileTreeItems = ({
	stackId,
	branchRef,
	branchDiff,
}: {
	stackId: string;
	branchRef: Array<number>;
	branchDiff: TreeChanges;
}): Array<FileTreeItem> =>
	branchDiff.changes.map((change) =>
		changeFileTreeItem({
			change,
			operand: {
				parent: branchFileParent({ stackId, branchRef }),
				path: change.path,
			},
		}),
	);

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

const mkCodeViewItem = (
	change: TreeChange,
	changesetKey: string,
	hunks: Array<DiffHunk>,
): CodeViewDiffItem | null => {
	const lineEnding = lineEndingForDiff(hunks[0]?.diff ?? "");
	const header = patchHeaderForChange(change, lineEnding);
	const combinedFilePatch = [header, ...hunks.map((hunk) => hunk.diff)].join(lineEnding);
	const version = Hash.string(combinedFilePatch);
	const parsed = parsePatchFiles(combinedFilePatch, String(version));

	return {
		type: "diff",
		id: codeViewItemId({ changesetKey, path: change.path }),
		version,
		// oxlint-disable-next-line typescript/no-non-null-assertion: There should always be exactly one result given our one parsed hunk.
		fileDiff: parsed[0]!.files[0]!,
	};
};

const DiffContents: FC<{
	changes: Array<TreeChange>;
	onViewerFileSelection: (selection: FileOperand) => void;
	outlineSelection: Operand;
	projectId: string;
	viewerRef: RefObject<CodeViewHandle<undefined> | null>;
}> = ({ changes, onViewerFileSelection, outlineSelection, projectId, viewerRef }) => {
	const treeChangeDiffs = useSuspenseQueries({
		queries: changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
	}).map((result) => result.data);

	const changesetKey = getChangesetKey(outlineSelection);
	const fileParent = Match.value(outlineSelection).pipe(
		Match.tags({
			Branch: ({ branchRef, stackId }) => branchFileParent({ branchRef, stackId }),
			ChangesSection: () => changesFileParent,
			Commit: ({ commitId, stackId }) => commitFileParent({ commitId, stackId }),
		}),
		Match.orElseAbsurd,
	);

	// CodeView only gives us back the CodeViewItem in custom renders, however we need this prior data
	// hence a reverse map by ID.
	const itemsMetadataMap = new Map<string, [TreeChange, UnifiedPatch]>();

	const items = Array.zip(changes, treeChangeDiffs).flatMap(([change, mdiff]) => {
		if (!mdiff) return [];

		const mitem = Match.value(mdiff).pipe(
			Match.when({ type: "Patch" }, (patch) =>
				mkCodeViewItem(change, changesetKey, patch.subject.hunks),
			),
			Match.when({ type: "Binary" }, () => mkCodeViewItem(change, changesetKey, [])),
			Match.orElse(() => null),
		);
		if (!mitem) return [];

		itemsMetadataMap.set(mitem.id, [change, mdiff]);

		return mitem;
	});

	const selectFileAtViewportTop = (scrollTop: number, viewer: CodeViewClass<undefined>) => {
		const activeItem = viewer
			.getRenderedItems()
			// oxlint-disable-next-line typescript/no-non-null-assertion: It can only be undefined if the item ID is invalid.
			.findLast((item) => viewer.getTopForItem(item.id)! <= scrollTop);

		// This can happen on very fast scroll.
		if (activeItem === undefined) return;

		onViewerFileSelection({
			parent: fileParent,
			path: codeViewItemIdPath({ changesetKey, id: activeItem.id }),
		});
	};

	return items.length === 0 ? (
		<p className="text-13">No changes.</p>
	) : (
		<CodeView
			ref={viewerRef}
			renderCustomHeader={(item) => {
				if (item.type === "file") throw new Error("Only diff items may be rendered");

				const path = itemsMetadataMap.get(item.id)?.[0].path;

				// CodeView may briefly hold onto stale snapshots of our data.
				if (path === undefined) return <div style={{ height: 38 }} />;

				return (
					<DiffFileHeader
						projectId={projectId}
						item={item}
						operand={fileOperand({
							parent: fileParent,
							path,
						})}
						path={path}
						hasDiff={item.fileDiff.hunks.length !== 0}
					/>
				);
			}}
			onScroll={selectFileAtViewportTop}
			className={styles.diffContents}
			items={items}
			options={{
				diffStyle: "unified",
				themeType: "system",
				stickyHeaders: true,
				layout: {
					paddingTop: 0,
					// Match --panel-padding.
					paddingBottom: 16,
					gap: 10,
				},
				// This appears to validate before our custom header has been slotted, in which case - if
				// our metrics are correct - we should see deltas in multiples of our custom header height
				// as defined in the metrics. We'll see an additional set of logs if there are other issues
				// with our metrics.
				__devOnlyValidateItemHeights: false,
				itemMetrics: {
					// Computed custom header height.
					diffHeaderHeight: 38,
					// Default spacing plus our 1px border.
					paddingBottom: 9,
				},
				unsafeCSS: `
          [data-code] {
            border-width: 0 1px 1px 1px;
            border-style: solid;
            border-color: var(--border-3);
            border-radius: 0 0 10px 10px;
          }
        `,
			}}
		/>
	);
};

type DiffFileHeaderProps = {
	projectId: string;
	item: CodeViewDiffItem;
	operand: Operand;
	path: string;
	hasDiff: boolean;
};

const DiffFileHeader: FC<DiffFileHeaderProps> = (p) => {
	const lastSepIdx = p.path.lastIndexOf("/");
	const mpathInit = lastSepIdx !== -1 ? p.path.slice(0, lastSepIdx + 1) : null;
	const pathLast = lastSepIdx !== -1 ? p.path.slice(lastSepIdx + 1) : p.path;

	const changeType = Match.value(p.item.fileDiff.type).pipe(
		Match.when("new", () => "Added"),
		Match.whenOr("change", "rename-changed", () => "Modified"),
		Match.when("rename-pure", () => "Renamed"),
		Match.when("deleted", () => "Deleted"),
		Match.exhaustive,
	);

	return (
		<OperationSourceC projectId={p.projectId} source={p.operand}>
			<header className={classes(styles.fileHeader, !p.hasDiff && styles.lone)}>
				<h4 className={classes("text-13", styles.filePath)}>
					{mpathInit}
					<span className={styles.pathLast}>{pathLast}</span>
				</h4>
				<span>{changeType}</span>
				<span>
					<span className={styles.fileDiffAdded}>+{p.item.fileDiff.additionLines.length}</span>{" "}
					<span className={styles.fileDiffDeleted}>-{p.item.fileDiff.deletionLines.length}</span>
				</span>
			</header>
		</OperationSourceC>
	);
};

const Header: FC<{
	projectId: string;
	selection: Operand;
}> = ({ projectId, selection }) => {
	const dispatch = useAppDispatch();
	const selectOutlineSource = (source: Operand) => {
		dispatch(projectActions.selectOutline({ projectId, selection: source }));
	};

	return Match.value(selection).pipe(
		Match.tagsExhaustive({
			Stack: () => null,
			Branch: ({ stackId, branchRef }) => {
				const decodedBranchRef = decodeRefName(branchRef);
				const source = branchOperand({ stackId, branchRef });

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
							<OperationSourceC
								projectId={projectId}
								source={source}
								onDragStart={() => selectOutlineSource(source)}
								render={<header className={styles.header} />}
							>
								<h3 className={classes("text-14", "text-semibold")}>{branchDetails.name}</h3>
								{branchDetails.prNumber != null && (
									<div className={classes("text-13", "text-bold", styles.pr)}>
										PR #{branchDetails.prNumber}
									</div>
								)}
							</OperationSourceC>
						)}
					</SuspenseQuery>
				);
			},
			ChangesSection: () => (
				<OperationSourceC
					projectId={projectId}
					source={changesSectionOperand}
					onDragStart={() => selectOutlineSource(changesSectionOperand)}
					render={<header className={styles.header} />}
				>
					<h3 className={classes("text-14", "text-semibold")}>Changes</h3>
				</OperationSourceC>
			),
			File: () => null,
			Commit: ({ commitId, stackId }) => {
				const source = commitOperand({ stackId, commitId });

				return (
					<SuspenseQuery {...commitDetailsWithLineStatsQueryOptions({ projectId, commitId })}>
						{({ data: commitDetails }) => (
							<OperationSourceC
								projectId={projectId}
								source={source}
								onDragStart={() => selectOutlineSource(source)}
								render={<header className={styles.header} />}
							>
								<Icon name="commit" />
								<h3 className={classes("text-14", "text-semibold")}>
									{commitTitle(commitDetails.commit.message)}
									{commitDetails.commit.hasConflicts && " ⚠️"}
								</h3>
								<span className={classes("text-13", styles.commitMeta)}>
									#{shortCommitId(commitDetails.commit.id)}
								</span>
							</OperationSourceC>
						)}
					</SuspenseQuery>
				);
			},
			Hunk: () => null,
		}),
	);
};

const FilesToggle: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const dispatch = useAppDispatch();
	const filesVisible = useAppSelector((state) => selectProjectFilesVisible(state, projectId));

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				className={getButtonClassName({})}
				aria-pressed={filesVisible}
				onClick={() => dispatch(projectActions.toggleFiles({ projectId }))}
			>
				Files
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup kbd={workspaceHotkeys.toggleFiles.hotkey} />}>
						{workspaceHotkeys.toggleFiles.meta.name}
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const FullscreenToggle: FC<{
	className?: string;
	fullscreen: boolean;
	onFullscreenChange: (fullscreen: boolean) => void;
}> = ({ className, fullscreen, onFullscreenChange }) => {
	const label = fullscreen ? "Exit fullscreen details" : "Fullscreen details";

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				aria-label={label}
				aria-pressed={fullscreen}
				className={className}
				onClick={() => onFullscreenChange(!fullscreen)}
			>
				<Icon name={fullscreen ? "fullscreen-exit" : "fullscreen-enter"} />
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup
						render={<TooltipPopup kbd={workspaceHotkeys.toggleDetailsFullscreen.hotkey} />}
					>
						{workspaceHotkeys.toggleDetailsFullscreen.meta.name}
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

const Diff: FC<{
	changes: Array<TreeChange>;
	filesVisible: boolean;
	filesItems: Array<FileTreeItem>;
	onFileSelection: (selection: FileOperand) => void;
	onViewerFileSelection: (selection: FileOperand) => void;
	outlineSelection: Operand;
	projectId: string;
	viewerRef: RefObject<CodeViewHandle<undefined> | null>;
}> = ({
	changes,
	filesVisible,
	filesItems,
	onFileSelection,
	onViewerFileSelection,
	outlineSelection,
	projectId,
	viewerRef,
}) => {
	const files = filesItems.map((item) => item.operand);

	const navigationIndex = buildNavigationIndex(files, (file) =>
		operandIdentityKey(fileOperand(file)),
	);

	return (
		<div className={classes(styles.diff, filesVisible && styles.diffWithFiles)}>
			{filesVisible && (
				<FilesTree
					id={"files" satisfies SelectionScope}
					data-selection-scope
					tabIndex={0}
					className={classes(styles.diffFiles, uiStyles.scrollerWithSeparator)}
					onFileSelection={onFileSelection}
					projectId={projectId}
					items={filesItems}
					navigationIndex={navigationIndex}
				/>
			)}

			<div
				id={"diff" satisfies SelectionScope}
				data-selection-scope
				// oxlint-disable-next-line jsx_a11y/no-noninteractive-tabindex -- Revisit this when we add hunk/line selection.
				tabIndex={0}
				className={styles.diffContentsContainer}
			>
				<Suspense fallback={<p className="text-13">Loading diff…</p>}>
					<DiffContents
						changes={changes}
						onViewerFileSelection={onViewerFileSelection}
						outlineSelection={outlineSelection}
						projectId={projectId}
						viewerRef={viewerRef}
					/>
				</Suspense>
			</div>
		</div>
	);
};

export const Details: FC<
	{
		detailsFullscreen: boolean;
		onDetailsFullscreenChange: (fullscreen: boolean) => void;
		outlineSelection: Operand | null;
	} & ComponentProps<"div">
> = ({
	detailsFullscreen,
	onDetailsFullscreenChange,
	outlineSelection: urgentOutlineSelection,
	...restProps
}) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const dispatch = useAppDispatch();
	const viewerRef = useRef<CodeViewHandle<undefined>>(null);
	const filesVisible = useAppSelector((state) => selectProjectFilesVisible(state, projectId));
	const outlineSelection = useDeferredValue(urgentOutlineSelection);

	const selectFile = (selection: FileOperand) => {
		dispatch(projectActions.selectFiles({ projectId, selection }));
	};

	const selectFileAndScrollDiff = (selection: FileOperand) => {
		if (!outlineSelection) return;

		selectFile(selection);

		const scrollTargetId = getScrollTargetId({
			changesetKey: getChangesetKey(outlineSelection),
			selection,
		});
		if (scrollTargetId === null) return;

		viewerRef.current?.scrollTo({ type: "item", id: scrollTargetId });
	};

	if (!outlineSelection || outlineSelection._tag === "Stack") return;

	return (
		<div
			{...restProps}
			className={classes(restProps.className, styles.container)}
			style={{ opacity: urgentOutlineSelection !== outlineSelection ? 0.5 : 1 }}
		>
			<div className={styles.headerWrap}>
				<Suspense fallback={<p className="text-13">Loading details…</p>}>
					<Header projectId={projectId} selection={outlineSelection} />

					{outlineSelection._tag === "Commit" && (
						<CommitDetailsContent projectId={projectId} commitId={outlineSelection.commitId} />
					)}
				</Suspense>

				<div className={styles.actions}>
					<FilesToggle />
					<FullscreenToggle
						className={getButtonClassName({ iconOnly: true })}
						fullscreen={detailsFullscreen}
						onFullscreenChange={onDetailsFullscreenChange}
					/>
				</div>
			</div>

			<Suspense
				fallback={<div className={classes(styles.loadingDiff, "text-13")}>Loading diff…</div>}
			>
				{(() => {
					const render = ({
						changes,
						filesItems,
					}: {
						changes: Array<TreeChange>;
						filesItems: Array<FileTreeItem>;
					}) => (
						<Diff
							key={operandIdentityKey(outlineSelection)}
							changes={changes}
							filesVisible={filesVisible}
							filesItems={filesItems}
							onFileSelection={selectFileAndScrollDiff}
							onViewerFileSelection={selectFile}
							outlineSelection={outlineSelection}
							projectId={projectId}
							viewerRef={viewerRef}
						/>
					);
					return Match.value(outlineSelection).pipe(
						Match.tag("Commit", (commit) => (
							<SuspenseQuery
								{...commitDetailsWithLineStatsQueryOptions({
									projectId,
									commitId: commit.commitId,
								})}
							>
								{({ data: commitDetails }) =>
									render({
										changes: commitDetails.changes,
										filesItems: getCommitFileTreeItems({ commit, commitDetails }),
									})
								}
							</SuspenseQuery>
						)),
						Match.tag("ChangesSection", () => (
							<SuspenseQuery {...changesInWorktreeQueryOptions(projectId)}>
								{({ data: worktreeChanges }) =>
									render({
										changes: worktreeChanges.changes,
										filesItems: getChangesFileTreeItems(worktreeChanges),
									})
								}
							</SuspenseQuery>
						)),
						Match.tag("Branch", ({ stackId, branchRef }) => (
							<SuspenseQuery
								{...branchDiffQueryOptions({ projectId, branch: decodeRefName(branchRef) })}
							>
								{({ data: branchDiff }) =>
									render({
										changes: branchDiff.changes,
										filesItems: getBranchFileTreeItems({ stackId, branchRef, branchDiff }),
									})
								}
							</SuspenseQuery>
						)),
						Match.orElse(() => null),
					);
				})()}
			</Suspense>
		</div>
	);
};
