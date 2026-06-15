import uiStyles from "#ui/components/ui.module.css";
import { SuspenseQuery } from "@suspensive/react-query";
import {
	branchDetailsQueryOptions,
	branchDiffQueryOptions,
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { commitBody, commitTitle, shortCommitId } from "#ui/commit.ts";
import {
	branchFileParent,
	changesFileParent,
	commitFileParent,
	FileOperand,
	fileOperand,
	hunkOperand,
	operandIdentityKey,
	type FileParent,
	type HunkOperand,
	type Operand,
} from "#ui/operands.ts";
import { projectActions, selectProjectFilesVisible } from "#ui/projects/state.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { Toolbar, Tooltip } from "@base-ui/react";
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
	type CodeViewLineSelection,
	parsePatchFiles,
} from "@pierre/diffs";
import { CodeView, type CodeViewHandle } from "@pierre/diffs/react";
import { useSuspenseQueries } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Hash, identity, Match } from "effect";
import { ComponentProps, FC, type RefObject, Suspense, useDeferredValue, useRef } from "react";
import styles from "./Details.module.css";
import { workspaceHotkeys } from "#ui/hotkeys.ts";
import {
	type SelectionScope,
	useDiffSelection,
	useNavigationIndexHotkeys,
} from "#ui/selection-scopes.ts";
import {
	FilesTree,
	changeFileTreeItem,
	conflictFileTreeItem,
	type FileTreeItem,
} from "#ui/routes/project/$id/workspace/FilesTree.tsx";
import {
	getDependencyCommitIds,
	getHunkDependencyDiffsByPath,
	contiguousSelectionByLine,
	contiguousSelectionsFromHunk,
	synthesizeFilePatch,
} from "#ui/hunk.ts";
import { buildIndexByKey, NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { showNativeContextMenu, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import { useFileMenuItems } from "#ui/routes/project/$id/workspace/useFileMenuItems.ts";

const codeViewItemId = ({ changesetKey, path }: { changesetKey: string; path: string }): string =>
	`${changesetKey}:${path}`;

const codeViewItemIdPath = ({ changesetKey, id }: { changesetKey: string; id: string }): string =>
	id.slice(changesetKey.length + 1);

const hunkOperandIdentityKey = (operand: HunkOperand): string =>
	operandIdentityKey(hunkOperand(operand));

const getCommitFileTreeItems = ({
	commitDetails,
}: {
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
				path,
			}),
		),
		...commitDetails.changes
			.filter((change) => !conflictedPathSet.has(change.path))
			.map((change) =>
				changeFileTreeItem({
					change,
					path: change.path,
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
			path: change.path,
		});
	});
};

const getBranchFileTreeItems = ({ branchDiff }: { branchDiff: TreeChanges }): Array<FileTreeItem> =>
	branchDiff.changes.map((change) =>
		changeFileTreeItem({
			change,
			path: change.path,
		}),
	);

const mkCodeViewItem = (
	change: TreeChange,
	changesetKey: string,
	hunks: Array<DiffHunk>,
): CodeViewDiffItem => {
	const combinedFilePatch = synthesizeFilePatch(change, hunks);
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

type DiffViewDeps = {
	fileParent: FileParent;
	changes: Array<TreeChange>;
	treeChangeDiffs: Array<UnifiedPatch | null>;
	changesetKey: string;
};

type DiffViewFile = {
	operand: FileOperand;
	item: CodeViewDiffItem;
	change: TreeChange;
	patch: UnifiedPatch | null;
	hunks: Array<DiffViewHunk>;
};

type DiffViewHunk = {
	operand: HunkOperand;
	selectedLines: CodeViewLineSelection;
};

type DiffView = {
	navigationIndex: NavigationIndex<HunkOperand>;
	items: Array<CodeViewDiffItem>;
	fileByItemId: Map<string, DiffViewFile>;
	fileByPath: Map<string, DiffViewFile>;
	fileByHunkKey: Map<string, DiffViewFile>;
	hunkByKey: Map<string, DiffViewHunk>;
};

/** Build relationships between our SDK data and Pierre's view. */
const getDiffView = ({
	fileParent,
	changes,
	treeChangeDiffs,
	changesetKey,
}: DiffViewDeps): DiffView => {
	const navigationIndex: NavigationIndex<HunkOperand> = {
		items: [],
		indexByKey: new Map(),
	};

	const items: Array<CodeViewDiffItem> = [];

	const fileByItemId = new Map<string, DiffViewFile>();
	const fileByPath = new Map<string, DiffViewFile>();
	const fileByHunkKey = new Map<string, DiffViewFile>();
	const hunkByKey = new Map<string, DiffViewHunk>();

	for (const [ci, change] of changes.entries()) {
		const mdiff = treeChangeDiffs[ci];

		const item = mkCodeViewItem(
			change,
			changesetKey,
			mdiff && "subject" in mdiff && "hunks" in mdiff.subject ? mdiff.subject.hunks : [],
		);

		items.push(item);

		const file: FileOperand = {
			parent: fileParent,
			path: change.path,
		};
		const diffViewFile: DiffViewFile = {
			operand: file,
			item,
			change,
			patch: mdiff ?? null,
			hunks: [],
		};

		fileByItemId.set(item.id, diffViewFile);
		fileByPath.set(change.path, diffViewFile);

		if (mdiff?.type === "Patch")
			for (const hunk of item.fileDiff.hunks)
				for (const selection of contiguousSelectionsFromHunk(hunk)) {
					const hunkOperand: HunkOperand = {
						parent: file,
						...selection,
						isResultOfBinaryToTextConversion: mdiff.subject.isResultOfBinaryToTextConversion,
					};
					const hunkKey = hunkOperandIdentityKey(hunkOperand);

					const len = navigationIndex.items.push(hunkOperand);
					navigationIndex.indexByKey.set(hunkKey, len - 1);

					const diffViewHunk: DiffViewHunk = {
						operand: hunkOperand,
						selectedLines: {
							id: item.id,
							range: hunkOperand.range,
						},
					};
					diffViewFile.hunks.push(diffViewHunk);
					fileByHunkKey.set(hunkKey, diffViewFile);
					hunkByKey.set(hunkKey, diffViewHunk);
				}
	}

	return {
		items,
		fileByItemId,
		fileByPath,
		fileByHunkKey,
		hunkByKey,
		navigationIndex,
	};
};

const DiffContents: FC<{
	selectionScopeRef: RefObject<HTMLDivElement | null>;
	onViewerFileSelection: (selection: string) => void;
	fileParent: FileParent;
	changesetKey: string;
	projectId: string;
	diffView: DiffView;
	viewerRef: RefObject<CodeViewHandle<undefined> | null>;
}> = ({
	selectionScopeRef,
	onViewerFileSelection,
	fileParent,
	changesetKey,
	projectId,
	diffView: { items, navigationIndex, hunkByKey, fileByHunkKey, fileByItemId },
	viewerRef,
}) => {
	const dispatch = useAppDispatch();

	const diffSelection = useDiffSelection(projectId, navigationIndex);
	const selectedRange = diffSelection
		? (hunkByKey.get(hunkOperandIdentityKey(diffSelection))?.selectedLines ?? null)
		: null;

	const selectDiff = (selection: HunkOperand) => {
		dispatch(projectActions.selectDiff({ projectId, selection }));

		const selectedRange = hunkByKey.get(hunkOperandIdentityKey(selection))?.selectedLines;
		if (!selectedRange) return;

		viewerRef.current?.scrollTo({
			type: "range",
			id: selectedRange.id,
			range: selectedRange.range,
			align: "nearest",
		});
	};

	useNavigationIndexHotkeys({
		navigationIndex,
		projectId,
		group: "Diff",
		selectionScope: "diff",
		select: selectDiff,
		selection: diffSelection,
		selectSectionPredicate: (hunk) => {
			const k = hunkOperandIdentityKey(hunk);
			// oxlint-disable-next-line typescript/no-non-null-assertion: Absurd.
			return hunkOperandIdentityKey(fileByHunkKey.get(k)!.hunks[0]!.operand) === k;
		},
		ref: selectionScopeRef,
		getKey: hunkOperandIdentityKey,
		operationSourceForItem: hunkOperand,
	});

	const selectFileAtViewportTop = (scrollTop: number, viewer: CodeViewClass<undefined>) => {
		const activeItem = viewer
			.getRenderedItems()
			// oxlint-disable-next-line typescript/no-non-null-assertion: It can only be undefined if the item ID is invalid.
			.findLast((item) => viewer.getTopForItem(item.id)! <= scrollTop);

		// This can happen on very fast scroll.
		if (activeItem === undefined) return;

		onViewerFileSelection(codeViewItemIdPath({ changesetKey, id: activeItem.id }));
	};

	// We currently only support selecting contiguous blocks.
	const handleLinesSelected = (sel: CodeViewLineSelection | null): void => {
		if (!sel) return void dispatch(projectActions.selectDiff({ projectId, selection: null }));

		const file = fileByItemId.get(sel.id);
		if (!file) throw new Error("Could not get file by item ID");
		if (file.patch?.type !== "Patch") throw new Error("File has no patch");

		const side = sel.range.endSide ?? sel.range.side;
		if (side === undefined) return;

		const selection = contiguousSelectionByLine({
			hunks: file.item.fileDiff.hunks,
			// The end range is more reliable in shift+click with preexisting selection scenarios.
			line: sel.range.end,
			side,
		});
		if (!selection) return;

		dispatch(
			projectActions.selectDiff({
				projectId,
				selection: {
					parent: {
						parent: fileParent,
						path: file.change.path,
					},
					...selection,
					isResultOfBinaryToTextConversion: file.patch.subject.isResultOfBinaryToTextConversion,
				},
			}),
		);
	};

	return items.length === 0 ? (
		<p className="text-13">No changes.</p>
	) : (
		<CodeView
			ref={viewerRef}
			renderCustomHeader={(item) => {
				if (item.type === "file") throw new Error("Only diff items may be rendered");

				const file = fileByItemId.get(item.id);

				// CodeView may briefly hold onto stale snapshots of our data.
				if (!file) return <div style={{ height: 38 }} />;

				return (
					<DiffFileHeader
						projectId={projectId}
						item={item}
						operand={file.operand}
						change={file.change}
						hasDiff={item.fileDiff.hunks.length !== 0}
					/>
				);
			}}
			onScroll={selectFileAtViewportTop}
			className={styles.diffContents}
			items={items}
			selectedLines={selectedRange}
			onSelectedLinesChange={handleLinesSelected}
			options={{
				diffStyle: "unified",
				themeType: "system",
				stickyHeaders: true,
				enableLineSelection: true,
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
	operand: FileOperand;
	change: TreeChange;
	hasDiff: boolean;
};

const DiffFileHeader: FC<DiffFileHeaderProps> = (p) => {
	const menuItems = useFileMenuItems({
		projectId: p.projectId,
		operand: p.operand,
		path: p.change.path,
		change: p.change,
	});

	const lastSepIdx = p.change.path.lastIndexOf("/");
	const mpathInit = lastSepIdx !== -1 ? p.change.path.slice(0, lastSepIdx + 1) : null;
	const pathLast = lastSepIdx !== -1 ? p.change.path.slice(lastSepIdx + 1) : p.change.path;

	const changeType = Match.value(p.item.fileDiff.type).pipe(
		Match.when("new", () => "Added"),
		Match.whenOr("change", "rename-changed", () => "Modified"),
		Match.when("rename-pure", () => "Renamed"),
		Match.when("deleted", () => "Deleted"),
		Match.exhaustive,
	);

	return (
		<OperationSourceC projectId={p.projectId} source={fileOperand(p.operand)}>
			<header
				onContextMenu={(event) => {
					void showNativeContextMenu(event, menuItems);
				}}
				className={classes(styles.fileHeader, !p.hasDiff && styles.lone)}
			>
				<h4 className={classes("text-13", styles.filePath)}>
					{mpathInit}
					<span className={styles.pathLast}>{pathLast}</span>
				</h4>
				<span>{changeType}</span>
				<span>
					<span className={styles.fileDiffAdded}>+{p.item.fileDiff.additionLines.length}</span>{" "}
					<span className={styles.fileDiffDeleted}>-{p.item.fileDiff.deletionLines.length}</span>
				</span>

				<Toolbar.Root aria-label="File actions">
					<Toolbar.Button
						aria-label="File menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getButtonClassName({ size: "small", variant: "ghost", iconOnly: true })}
					>
						<Icon name="kebab" />
					</Toolbar.Button>
				</Toolbar.Root>
			</header>
		</OperationSourceC>
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
				const decodedBranchRef = decodeBytes(branchRef);

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
									<div className={classes("text-13", "text-bold", styles.pr)}>
										PR #{branchDetails.prNumber}
									</div>
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
			Commit: ({ commitId }) => (
				<SuspenseQuery {...commitDetailsWithLineStatsQueryOptions({ projectId, commitId })}>
					{({ data: commitDetails }) => (
						<header className={styles.header}>
							<Icon name="commit" />
							<h3 className={classes("text-14", "text-semibold")}>
								{commitTitle(commitDetails.commit.message) ?? "(no message)"}
								{commitDetails.commit.hasConflicts && " ⚠️"}
							</h3>
							<span className={classes("text-13", styles.commitMeta)}>
								#{shortCommitId(commitDetails.commit.id)}
							</span>
						</header>
					)}
				</SuspenseQuery>
			),
			Hunk: () => null,
		}),
	);

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
				<Icon name={fullscreen ? "unfold-less-horizontal" : "unfold-more-horizontal"} />
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
			}).format(commitDetails.commit.authoredAt);

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
	onFileSelection: (selection: string) => void;
	outlineSelection: Operand;
	projectId: string;
}> = ({ changes, filesVisible, filesItems, onFileSelection, outlineSelection, projectId }) => {
	const selectionScopeRef = useRef<HTMLDivElement>(null);
	const viewerRef = useRef<CodeViewHandle<undefined>>(null);
	const dispatch = useAppDispatch();
	const files = filesItems.map((item) => item.path);
	const filesIndexByKey = buildIndexByKey(files, identity);

	const changesetKey = Match.value(outlineSelection).pipe(
		Match.tags({
			Branch: ({ branchRef }) => decodeBytes(branchRef),
			ChangesSection: () => "changes",
			Commit: ({ commitId }) => commitId,
		}),
		Match.orElseAbsurd,
	);
	const fileParent = Match.value(outlineSelection).pipe(
		Match.tags({
			Branch: ({ branchRef, stackId }) => branchFileParent({ branchRef, stackId }),
			ChangesSection: () => changesFileParent,
			Commit: ({ commitId, stackId }) => commitFileParent({ commitId, stackId }),
		}),
		Match.orElseAbsurd,
	);

	const treeChangeDiffs = useSuspenseQueries({
		queries: changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
	}).map((result) => result.data);

	const diffView = getDiffView({
		fileParent,
		changes,
		treeChangeDiffs,
		changesetKey,
	});

	const selectFileAndNavigateDiff = (selection: string) => {
		onFileSelection(selection);

		dispatch(
			projectActions.selectDiff({
				projectId,
				selection: diffView.fileByPath.get(selection)?.hunks[0]?.operand ?? null,
			}),
		);

		viewerRef.current?.scrollTo({
			type: "item",
			id: codeViewItemId({ changesetKey, path: selection }),
		});
	};

	return (
		<div className={classes(styles.diff, filesVisible && styles.diffWithFiles)}>
			{filesVisible && (
				<FilesTree
					id={"files" satisfies SelectionScope}
					data-selection-scope
					tabIndex={0}
					className={classes(styles.diffFiles, uiStyles.scrollerWithSeparator)}
					onFileSelection={selectFileAndNavigateDiff}
					projectId={projectId}
					items={filesItems}
					navigationIndex={{ items: files, indexByKey: filesIndexByKey }}
					fileParent={fileParent}
				/>
			)}

			<div
				id={"diff" satisfies SelectionScope}
				data-selection-scope
				// oxlint-disable-next-line jsx_a11y/no-noninteractive-tabindex -- Revisit this when we add hunk/line selection.
				tabIndex={0}
				className={styles.diffContentsContainer}
				ref={selectionScopeRef}
			>
				<DiffContents
					onViewerFileSelection={onFileSelection}
					fileParent={fileParent}
					changesetKey={changesetKey}
					projectId={projectId}
					diffView={diffView}
					selectionScopeRef={selectionScopeRef}
					viewerRef={viewerRef}
				/>
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
	const filesVisible = useAppSelector((state) => selectProjectFilesVisible(state, projectId));
	const outlineSelection = useDeferredValue(urgentOutlineSelection);

	const selectFile = (selection: string) => {
		dispatch(projectActions.selectFiles({ projectId, selection }));
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
							onFileSelection={selectFile}
							outlineSelection={outlineSelection}
							projectId={projectId}
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
										filesItems: getCommitFileTreeItems({ commitDetails }),
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
						Match.tag("Branch", ({ branchRef }) => (
							<SuspenseQuery
								{...branchDiffQueryOptions({ projectId, branch: decodeBytes(branchRef) })}
							>
								{({ data: branchDiff }) =>
									render({
										changes: branchDiff.changes,
										filesItems: getBranchFileTreeItems({ branchDiff }),
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
