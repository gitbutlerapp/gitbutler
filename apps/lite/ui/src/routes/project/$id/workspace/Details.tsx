import uiStyles from "#ui/components/ui.module.css";
import { SuspenseQuery } from "@suspensive/react-query";
import { useMergeReview, useSetReviewDraftiness, useUpdateReview } from "#ui/api/mutations.ts";
import {
	branchDetailsQueryOptions,
	branchDiffQueryOptions,
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	getReviewMergeStatusQueryOptions,
	getReviewQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { commitBody, commitTitle, shortCommitId } from "#ui/commit.ts";
import {
	branchFileParent,
	uncommittedChangesFileParent,
	commitFileParent,
	FileOperand,
	fileOperand,
	hunkOperand,
	operandIdentityKey,
	type FileParent,
	type HunkOperand,
	type Operand,
} from "#ui/operands.ts";
import {
	projectActions,
	selectProjectFilesVisible,
	selectProjectPreferredDiffStyle,
	type DiffStyle,
} from "#ui/projects/state.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { Kbd } from "#ui/components/Kbd.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { Field, Toggle, ToggleGroup, Toolbar, Tooltip } from "@base-ui/react";
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
import { useSuspenseQueries, useSuspenseQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Hash, identity, Match } from "effect";
import {
	ComponentProps,
	FC,
	type RefObject,
	SubmitEventHandler,
	Suspense,
	useId,
	useLayoutEffect,
	useRef,
	useState,
} from "react";
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels";
import styles from "./Details.module.css";
import { diffHotkeys, pullRequestHotkeys, workspaceHotkeys } from "#ui/hotkeys.ts";
import { useHotkey, useHotkeys } from "@tanstack/react-hotkeys";
import {
	type SelectionScope,
	useDiffSelection,
	useNavigationIndexHotkeys,
} from "#ui/selection-scopes.ts";
import { FilesTree } from "#ui/routes/project/$id/workspace/FilesTree.tsx";
import { changeFileTreeItem, conflictFileTreeItem, type FileTreeItem } from "./file-tree.ts";
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
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";

type BranchTab = "diff" | "pr";

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
	diffStyle: DiffStyle;
	viewerRef: RefObject<CodeViewHandle<undefined> | null>;
}> = ({
	selectionScopeRef,
	onViewerFileSelection,
	fileParent,
	changesetKey,
	projectId,
	diffView: { items, navigationIndex, hunkByKey, fileByHunkKey, fileByItemId },
	diffStyle,
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
				diffStyle,
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
            border-radius: 0 0 10px 10px;
          }

          [data-diff] {
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

				<Toolbar.Root aria-label="File actions" className={styles.fileHeaderActions}>
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

const Title: FC<{
	bodyCollapsed: boolean;
	bodyId: string;
	onBodyCollapsedChange: (collapsed: boolean) => void;
	projectId: string;
	selection: Operand;
}> = ({ bodyCollapsed, bodyId, onBodyCollapsedChange, projectId, selection }) =>
	Match.value(selection).pipe(
		Match.tagsExhaustive({
			Stack: () => null,
			Branch: ({ branchRef }) => (
				<SuspenseQuery
					{...branchDetailsQueryOptions({
						projectId,
						// https://linear.app/gitbutler/issue/GB-1226/unify-branch-identifiers
						branchName: decodeBytes(branchRef).replace(/^refs\/heads\//, ""),
						remote: null,
					})}
				>
					{({ data: branchDetails }) => (
						<div className={styles.title}>
							<Icon name="branch" />
							<h3 className={classes("text-15", "text-semibold")}>{branchDetails.name}</h3>
						</div>
					)}
				</SuspenseQuery>
			),
			UncommittedChanges: () => (
				<div className={styles.title}>
					<Icon name="file-diff" />
					<h3 className={classes("text-15", "text-semibold")}>Uncommitted changes</h3>
				</div>
			),
			File: () => null,
			Commit: ({ commitId }) => (
				<SuspenseQuery {...commitDetailsWithLineStatsQueryOptions({ projectId, commitId })}>
					{({ data: commitDetails }) => (
						<div className={styles.title}>
							<Icon name="commit" />
							<h3 className={classes("text-15", "text-semibold")}>
								{commitTitle(commitDetails.commit.message) ?? "(no message)"}
								{commitDetails.commit.hasConflicts && " ⚠️"}
							</h3>
							{commitBody(commitDetails.commit.message) !== undefined && (
								<Tooltip.Root>
									<Tooltip.Trigger
										aria-controls={bodyId}
										aria-expanded={!bodyCollapsed}
										aria-label={bodyCollapsed ? "Expand commit body" : "Collapse commit body"}
										className={getButtonClassName({
											variant: "ghost",
											iconOnly: true,
										})}
										onClick={() => onBodyCollapsedChange(!bodyCollapsed)}
									>
										<Icon name={bodyCollapsed ? "uncollapse" : "collapse"} />
									</Tooltip.Trigger>
									<Tooltip.Portal>
										<Tooltip.Positioner sideOffset={4}>
											<Tooltip.Popup render={<TooltipPopup />}>
												{bodyCollapsed ? "Expand commit body" : "Collapse commit body"}
											</Tooltip.Popup>
										</Tooltip.Positioner>
									</Tooltip.Portal>
								</Tooltip.Root>
							)}
						</div>
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

const DiffStyleToggle: FC<{
	className?: string;
	diffStyle: DiffStyle;
	onDiffStyleChange: (diffStyle: DiffStyle) => void;
}> = ({ className, diffStyle, onDiffStyleChange }) => (
	<Tooltip.Root>
		<Tooltip.Trigger
			render={
				<ToggleGroup
					className={className}
					render={<ToggleGroupStyles />}
					aria-label={diffHotkeys.toggleDiffStyle.meta.name}
					value={[diffStyle]}
					onValueChange={(value: Array<DiffStyle>) => {
						const head = value[0];
						if (head === undefined) return;
						onDiffStyleChange(head);
					}}
				/>
			}
		>
			<Toggle render={<ToggleStyles />} value={"split" satisfies DiffStyle}>
				Split
			</Toggle>
			<Toggle render={<ToggleStyles />} value={"unified" satisfies DiffStyle}>
				Unified
			</Toggle>
		</Tooltip.Trigger>
		<Tooltip.Portal>
			<Tooltip.Positioner sideOffset={4}>
				<Tooltip.Popup render={<TooltipPopup kbd={diffHotkeys.toggleDiffStyle.hotkey} />}>
					{diffHotkeys.toggleDiffStyle.meta.name}
				</Tooltip.Popup>
			</Tooltip.Positioner>
		</Tooltip.Portal>
	</Tooltip.Root>
);

const FullWindowToggle: FC<{
	className?: string;
	fullWindow: boolean;
	onFullWindowChange: (fullWindow: boolean) => void;
}> = ({ className, fullWindow, onFullWindowChange }) => {
	const label = fullWindow ? "Exit full window details" : "Full window details";

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				aria-label={label}
				aria-pressed={fullWindow}
				className={className}
				onClick={() => onFullWindowChange(!fullWindow)}
			>
				<Icon name={fullWindow ? "fullscreen-exit" : "fullscreen-enter"} />
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup
						render={<TooltipPopup kbd={workspaceHotkeys.toggleDetailsFullWindow.hotkey} />}
					>
						{workspaceHotkeys.toggleDetailsFullWindow.meta.name}
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const isMac = window.lite.platform === "darwin";

const CommitDetailsContent: FC<{
	bodyCollapsed: boolean;
	bodyId: string;
	projectId: string;
	commitId: string;
}> = ({ bodyCollapsed, bodyId, projectId, commitId }) => {
	const { data: commitDetails } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId }),
	);

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
				<p
					id={bodyId}
					className={classes(
						"text-monospace",
						"text-body",
						styles.commitMessageBody,
						bodyCollapsed && styles.commitMessageBodyCollapsed,
					)}
				>
					{body}
				</p>
			)}
			<div className={classes("text-13", styles.commitDetailsMeta)}>
				<img
					src={commitDetails.commit.author.gravatarUrl}
					className={styles.avatar}
					alt="Commit author avatar"
				/>
				<span>
					<span title={commitDetails.commit.author.email}>{commitDetails.commit.author.name}</span>{" "}
					at {fmtDate}
				</span>
				<span>
					{shortCommitId(commitDetails.commit.changeId)} ({shortCommitId(commitDetails.commit.id)})
				</span>
			</div>
		</>
	);
};

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
			UncommittedChanges: () => "uncommittedChanges",
			Commit: ({ commitId }) => commitId,
		}),
		Match.orElseAbsurd,
	);
	const fileParent = Match.value(outlineSelection).pipe(
		Match.tags({
			Branch: ({ branchRef, stackId }) => branchFileParent({ branchRef, stackId }),
			UncommittedChanges: () => uncommittedChangesFileParent,
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

	const preferredDiffStyle = useAppSelector((state) =>
		selectProjectPreferredDiffStyle(state, projectId),
	);
	const diffContentsEl = useRef<HTMLElement | null>(null);
	const [canUseSplitDiff, setCanUseSplitDiff] = useState<boolean | undefined>();

	const toggleDiffStyle = () => dispatch(projectActions.togglePreferredDiffStyle({ projectId }));

	useHotkeys([
		{
			hotkey: diffHotkeys.toggleDiffStyle.hotkey,
			callback: toggleDiffStyle,
			options: {
				conflictBehavior: "allow",
				enabled: canUseSplitDiff,
				meta: diffHotkeys.toggleDiffStyle.meta,
				ignoreInputs: true,
			},
		},
	]);

	useLayoutEffect(() => {
		const el = diffContentsEl.current;
		if (!el) return;

		const measureCanUseSplitDiff = () => el.getBoundingClientRect().width >= 700;

		setCanUseSplitDiff(measureCanUseSplitDiff());

		const resizeObserver = new ResizeObserver(() => {
			setCanUseSplitDiff(measureCanUseSplitDiff());
		});
		resizeObserver.observe(el);

		return () => resizeObserver.disconnect();
	}, [diffContentsEl]);

	const layoutId = `project=${projectId}:details`;
	const diffLayout = useDefaultLayout({
		id: layoutId,
		panelIds: [...(filesVisible ? ["files"] : []), "diff"],
	});

	return (
		<div className={styles.diffTab}>
			<div className={styles.actions}>
				<FilesToggle />
				{canUseSplitDiff && (
					<DiffStyleToggle
						className={styles.actionsRight}
						diffStyle={preferredDiffStyle}
						onDiffStyleChange={(diffStyle) =>
							dispatch(projectActions.setPreferredDiffStyle({ projectId, diffStyle }))
						}
					/>
				)}
			</div>

			<Group
				id={layoutId}
				className={styles.panels}
				defaultLayout={diffLayout.defaultLayout}
				onLayoutChanged={diffLayout.onLayoutChanged}
			>
				{filesVisible && (
					<>
						<Panel
							id="files"
							className={styles.panel}
							defaultSize={250}
							minSize={180}
							groupResizeBehavior="preserve-pixel-size"
						>
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
						</Panel>
						<Separator className={styles.resizeHandle} />
					</>
				)}

				<Panel id="diff" minSize={300} className={styles.panel}>
					<div
						id={"diff" satisfies SelectionScope}
						data-selection-scope
						// oxlint-disable-next-line jsx_a11y/no-noninteractive-tabindex -- Revisit this when we add hunk/line selection.
						tabIndex={0}
						className={styles.diffContentsContainer}
						ref={useMergedRefs(selectionScopeRef, diffContentsEl)}
					>
						<DiffContents
							onViewerFileSelection={onFileSelection}
							fileParent={fileParent}
							changesetKey={changesetKey}
							projectId={projectId}
							diffView={diffView}
							diffStyle={canUseSplitDiff ? preferredDiffStyle : "unified"}
							selectionScopeRef={selectionScopeRef}
							viewerRef={viewerRef}
						/>
					</div>
				</Panel>
			</Group>
		</div>
	);
};

const PullRequestForm: FC<{
	projectId: string;
	reviewId: number;
	title: string;
	body: string | null;
}> = ({ projectId, reviewId, title, body }) => {
	const updateReview = useUpdateReview();
	const formRef = useRef<HTMLFormElement | null>(null);
	const [draftTitle, setDraftTitle] = useState<string | null>(null);
	const [draftBody, setDraftBody] = useState<string | null>(null);
	const canSubmit = (draftTitle === null || draftTitle.trim() !== "") && !updateReview.isPending;

	const reset = () => {
		setDraftTitle(title);
		setDraftBody(body);
	};

	const submit: SubmitEventHandler<HTMLFormElement> = (event) => {
		event.preventDefault();
		if (!canSubmit) return;

		updateReview.mutate({
			projectId,
			reviewId,
			title: draftTitle,
			body: draftBody,
			state: null,
			targetBase: null,
		});
	};

	useHotkey(pullRequestHotkeys.update.hotkey, () => formRef.current?.requestSubmit(), {
		conflictBehavior: "allow",
		ignoreInputs: false,
		meta: pullRequestHotkeys.update.meta,
		target: formRef,
	});

	return (
		<form ref={formRef} className={styles.prForm} onSubmit={submit}>
			<Field.Root className={styles.prFormField}>
				<Field.Label className="text-14">Title</Field.Label>
				<Field.Control
					className={classes("text-15 text-semibold", styles.prTitleInput)}
					onChange={(event) => setDraftTitle(event.currentTarget.value)}
					placeholder="Title"
					required
					value={draftTitle ?? title}
				/>
			</Field.Root>

			<Field.Root className={styles.prFormField}>
				<Field.Label className="text-14">Description</Field.Label>
				<Field.Control
					render={<textarea />}
					className={classes("text-14 text-body text-monospace", styles.prDescriptionInput)}
					onChange={(event) => setDraftBody(event.currentTarget.value)}
					placeholder="Description"
					value={draftBody ?? body ?? ""}
				/>
			</Field.Root>

			<div className={styles.prFormActions}>
				<button
					className={getButtonClassName({})}
					disabled={updateReview.isPending}
					onClick={reset}
					type="button"
				>
					Reset
				</button>
				<button
					className={getButtonClassName({ variant: "pop" })}
					disabled={!canSubmit}
					type="submit"
				>
					{updateReview.isPending && <Icon name="spinner" />}
					Update
					<Kbd hotkey={pullRequestHotkeys.update.hotkey} />
				</button>
			</div>
		</form>
	);
};

const PullRequestPrimaryAction: FC<{
	projectId: string;
	reviewId: number;
}> = ({ projectId, reviewId }) => {
	const [{ data: review }, { data: mergeStatus }] = useSuspenseQueries({
		queries: [
			getReviewQueryOptions({ projectId, reviewId }),
			getReviewMergeStatusQueryOptions({ projectId, reviewId }),
		],
	});

	const mergeReview = useMergeReview();
	const setReviewDraftiness = useSetReviewDraftiness();
	const isPending = mergeReview.isPending || setReviewDraftiness.isPending;

	const canUsePrimaryAction = (review.draft || mergeStatus.isMergeable) && !isPending;

	const primaryAction = () => {
		if (!canUsePrimaryAction) return;
		if (review.draft) {
			setReviewDraftiness.mutate({ projectId, reviewId, draft: false });
			return;
		}
		mergeReview.mutate({ projectId, reviewId, mergeMethod: null });
	};

	return (
		<button
			className={getButtonClassName({ variant: "pop" })}
			disabled={!canUsePrimaryAction}
			onClick={primaryAction}
			type="button"
		>
			{isPending && <Icon name="spinner" />}
			{review.draft ? "Mark as Ready" : "Merge"}
		</button>
	);
};

export const Details: FC<
	{
		detailsFullWindow: boolean;
		onDetailsFullWindowChange: (fullWindow: boolean) => void;
		outlineSelection: Operand | null;
	} & ComponentProps<"div">
> = ({ detailsFullWindow, onDetailsFullWindowChange, outlineSelection, ...restProps }) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const dispatch = useAppDispatch();
	const filesVisible = useAppSelector((state) => selectProjectFilesVisible(state, projectId));
	const [commitBodyCollapsed, setCommitBodyCollapsed] = useState(true);
	const [branchTab, setBranchTab] = useState<BranchTab>("diff");
	const commitBodyId = useId();

	const selectFile = (selection: string) => {
		dispatch(projectActions.selectFiles({ projectId, selection }));
	};

	if (!outlineSelection) return;

	return (
		<div {...restProps} className={classes(restProps.className, styles.container)}>
			<div className={styles.headerWrap}>
				<div className={styles.titleRow}>
					<div className={classes(detailsFullWindow && isMac && styles.titleRowMacSpacer)} />
					<Title
						bodyCollapsed={commitBodyCollapsed}
						bodyId={commitBodyId}
						onBodyCollapsedChange={setCommitBodyCollapsed}
						projectId={projectId}
						selection={outlineSelection}
					/>
					<FullWindowToggle
						className={classes(styles.titleRowActions, getButtonClassName({ iconOnly: true }))}
						fullWindow={detailsFullWindow}
						onFullWindowChange={onDetailsFullWindowChange}
					/>
				</div>

				{outlineSelection._tag === "Branch" && (
					<div className={styles.tabsRow}>
						<ToggleGroup
							render={<ToggleGroupStyles />}
							value={[branchTab]}
							onValueChange={(value: Array<BranchTab>) => {
								const head = value[0];
								if (head === undefined) return;
								setBranchTab(head);
							}}
							aria-label="Branch tab"
						>
							<Toggle render={<ToggleStyles />} value={"diff" satisfies BranchTab}>
								Diff
							</Toggle>
							<Toggle render={<ToggleStyles />} value={"pr" satisfies BranchTab}>
								Pull Request
							</Toggle>
						</ToggleGroup>

						<Suspense>
							<SuspenseQuery
								{...branchDetailsQueryOptions({
									projectId,
									// https://linear.app/gitbutler/issue/GB-1226/unify-branch-identifiers
									branchName: decodeBytes(outlineSelection.branchRef).replace(/^refs\/heads\//, ""),
									remote: null,
								})}
							>
								{({ data: branchDetails }) =>
									branchDetails.prNumber !== null && (
										<div className={styles.tabsRowRight}>
											<PullRequestPrimaryAction
												projectId={projectId}
												reviewId={branchDetails.prNumber}
											/>
										</div>
									)
								}
							</SuspenseQuery>
						</Suspense>
					</div>
				)}

				{outlineSelection._tag === "Commit" && (
					<CommitDetailsContent
						bodyCollapsed={commitBodyCollapsed}
						bodyId={commitBodyId}
						projectId={projectId}
						commitId={outlineSelection.commitId}
					/>
				)}
			</div>

			<Suspense fallback={<div className={classes(styles.loadingTab, "text-13")}>Loading…</div>}>
				{(() => {
					const renderDiff = ({
						changes,
						filesItems,
					}: {
						changes: Array<TreeChange>;
						filesItems: Array<FileTreeItem>;
					}) => (
						<Diff
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
									renderDiff({
										changes: commitDetails.changes,
										filesItems: getCommitFileTreeItems({ commitDetails }),
									})
								}
							</SuspenseQuery>
						)),
						Match.tag("UncommittedChanges", () => (
							<SuspenseQuery {...changesInWorktreeQueryOptions(projectId)}>
								{({ data: worktreeChanges }) =>
									renderDiff({
										changes: worktreeChanges.changes,
										filesItems: getChangesFileTreeItems(worktreeChanges),
									})
								}
							</SuspenseQuery>
						)),
						Match.tag("Branch", ({ branchRef }) =>
							branchTab === "pr" ? (
								<SuspenseQuery
									{...branchDetailsQueryOptions({
										projectId,
										// https://linear.app/gitbutler/issue/GB-1226/unify-branch-identifiers
										branchName: decodeBytes(branchRef).replace(/^refs\/heads\//, ""),
										remote: null,
									})}
								>
									{({ data: branchDetails }) => {
										const reviewId = branchDetails.prNumber;

										return (
											<div className={styles.prTab}>
												{reviewId === null ? (
													<p className="text-13">No pull request found.</p>
												) : (
													<SuspenseQuery {...getReviewQueryOptions({ projectId, reviewId })}>
														{({ data: review }) => (
															<PullRequestForm
																key={reviewId}
																body={review.body}
																projectId={projectId}
																reviewId={reviewId}
																title={review.title}
															/>
														)}
													</SuspenseQuery>
												)}
											</div>
										);
									}}
								</SuspenseQuery>
							) : (
								<SuspenseQuery
									{...branchDiffQueryOptions({ projectId, branch: decodeBytes(branchRef) })}
								>
									{({ data: branchDiff }) =>
										renderDiff({
											changes: branchDiff.changes,
											filesItems: getBranchFileTreeItems({ branchDiff }),
										})
									}
								</SuspenseQuery>
							),
						),
						Match.orElse(() => null),
					);
				})()}
			</Suspense>
		</div>
	);
};
