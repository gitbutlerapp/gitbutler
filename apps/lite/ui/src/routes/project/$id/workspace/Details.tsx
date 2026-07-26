import uiStyles from "#ui/components/ui.module.css";
import { SuspenseQuery } from "@suspensive/react-query";
import {
	useMergeReview,
	useOpenInProgram,
	usePublishReview,
	useSaveGUISettings,
	useSetReviewAutoMerge,
	useSetReviewDraftiness,
	useUpdateReview,
} from "#ui/api/mutations.ts";
import {
	branchDetailsQueryOptions,
	branchDiffQueryOptions,
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	forgeInfoOptions,
	guiSettingsQueryOptions,
	getReviewMergeStatusQueryOptions,
	headInfoQueryOptions,
	listCIChecksQueryOptions,
	listEditorsQueryOptions,
	listReviewsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { branchDetailsParams } from "#ui/branch.ts";
import { commitBody, commitTitle, shortCommitId } from "#ui/commit.ts";
import {
	branchFileParent,
	commitFileParent,
	type FileOperand,
	fileOperand,
	hunkOperand,
	operandIdentityKey,
	type FileParent,
	type HunkOperand,
	type Operand,
} from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import { Badge } from "#ui/components/Badge.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import {
	FieldControlStyles,
	FieldLabelStyles,
	FieldRootStyles,
	FieldTextareaStyles,
} from "#ui/components/Field.tsx";
import { Field, Toggle, ToggleGroup, Toolbar, Tooltip } from "@base-ui/react";
import type {
	CiCheck,
	CommitDetails,
	DiffHunk,
	TreeChange,
	TreeChanges,
	UnifiedPatch,
} from "@gitbutler/but-sdk";
import {
	type CodeViewDiffItem,
	type CodeView as CodeViewClass,
	type CodeViewLineSelection,
	parsePatchFiles,
} from "@pierre/diffs";
import { CodeView, type CodeViewHandle } from "@pierre/diffs/react";
import { useQuery, useSuspenseQueries, useSuspenseQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Match } from "effect";
import {
	type ComponentProps,
	type FC,
	type MouseEvent,
	type RefObject,
	type SubmitEventHandler,
	Suspense,
	useId,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels";
import styles from "./Details.module.css";
import { diffHotkeys, pullRequestHotkeys, workspaceHotkeys } from "#ui/hotkeys.ts";
import { useHotkey, useHotkeys } from "@tanstack/react-hotkeys";
import { type SelectionScope, useNavigationIndexHotkeys } from "#ui/selection-scopes.ts";
import { FilesTree } from "#ui/routes/project/$id/workspace/FilesTree.tsx";
import { TopLeftControls } from "#ui/routes/project/$id/workspace/TopLeftControls.tsx";
import {
	changeFileRowItem,
	conflictFileRowItem,
	getChangesFileRowItems,
	type FileRowItem,
} from "./file-row.ts";
import {
	contiguousSelectionByLine,
	contiguousSelectionsFromHunk,
	rangeFromLineGroups,
	synthesizeFilePatch,
} from "#ui/hunk.ts";
import { buildIndexByKey, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { showNativeContextMenu, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import { useFileMenuItems } from "#ui/routes/project/$id/workspace/useFileMenuItems.ts";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { Checkbox } from "#ui/components/Checkbox.tsx";
import type { GUISettings } from "#electron/settings.ts";
import { defaultSettings } from "#ui/settings.ts";
import type { AggregateCIChecks } from "#ui/ci.ts";
import type { IconName } from "#ui/components/iconNames.ts";
import { draftPRQueryOptions, usePersistDraftPR } from "#ui/pr.ts";
import { combineHashes, hash } from "#ui/hash.ts";
import { assert } from "#ui/assert.ts";
import {
	type DiffLineContextMenuTarget,
	useDiffLineContextMenu,
} from "./diff-line-context-menu.ts";
import { useDiffHunkDrag } from "./diff-hunk-drag.ts";
import type { DiffLineTarget } from "./diff-line-target.ts";
import { useHunkMenuItems } from "./useHunkMenuItems.ts";

type BranchTab = "diff" | "pr";

// This must be unique as to not collide with other IDs, and stable because it's
// stored in local storage.
type PanelId = "files-panel" | "diff-panel";

const diffDefaults = {
	diffBackground: true,
	diffOverflow: "scroll",
	diffStyle: "split",
} satisfies Partial<GUISettings>;

const codeViewItemId = ({ changesetKey, path }: { changesetKey: string; path: string }): string =>
	`${changesetKey}:${path}`;

const codeViewItemIdPath = ({ changesetKey, id }: { changesetKey: string; id: string }): string =>
	id.slice(changesetKey.length + 1);

const hunkOperandIdentityKey = (operand: HunkOperand): string =>
	operandIdentityKey(hunkOperand(operand));

const getCommitFileRowItems = ({
	commitDetails,
}: {
	commitDetails: CommitDetails;
}): Array<FileRowItem> => {
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
			conflictFileRowItem({
				path,
			}),
		),
		...commitDetails.changes
			.filter((change) => !conflictedPathSet.has(change.path))
			.map((change) =>
				changeFileRowItem({
					change,
					path: change.path,
					dependencyCommitIds: [],
				}),
			),
	];
};

const getBranchFileRowItems = ({ branchDiff }: { branchDiff: TreeChanges }): Array<FileRowItem> =>
	branchDiff.changes.map((change) =>
		changeFileRowItem({
			change,
			path: change.path,
			dependencyCommitIds: [],
		}),
	);

const mkCodeViewItem = (
	change: TreeChange,
	changesetKey: string,
	hunks: Array<DiffHunk>,
): CodeViewDiffItem => {
	const combinedFilePatch = synthesizeFilePatch(change, hunks);
	const version = hash(combinedFilePatch);
	const parsed = parsePatchFiles(combinedFilePatch, String(version));

	return {
		type: "diff",
		id: codeViewItemId({ changesetKey, path: change.path }),
		version,
		// There should always be exactly one result given our one parsed hunk.
		fileDiff: assert(assert(parsed[0]).files[0]),
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

		if (mdiff?.type === "Patch") {
			for (const hunk of item.fileDiff.hunks) {
				for (const selection of contiguousSelectionsFromHunk(hunk)) {
					const range = rangeFromLineGroups(selection.lineGroups);
					if (!range) continue;

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
							range,
						},
					};
					diffViewFile.hunks.push(diffViewHunk);
					fileByHunkKey.set(hunkKey, diffViewFile);
					hunkByKey.set(hunkKey, diffViewHunk);
				}
			}
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
	diffBackgrounds?: GUISettings["diffBackground"];
	diffOverflow?: GUISettings["diffOverflow"];
	diffStyle?: GUISettings["diffStyle"];
	viewerRef: RefObject<CodeViewHandle<undefined> | null>;
	didScrollToViaFileRef: RefObject<boolean>;
}> = ({
	selectionScopeRef,
	onViewerFileSelection,
	fileParent,
	changesetKey,
	projectId,
	diffView: { items, navigationIndex, hunkByKey, fileByHunkKey, fileByItemId },
	diffBackgrounds,
	diffOverflow,
	diffStyle,
	viewerRef,
	didScrollToViaFileRef,
}) => {
	const [collapsedItems, setCollapsedItems] = useState<Set<string>>(new Set());
	const dispatch = useAppDispatch();
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: preferredEditor } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => editors?.find((editor) => editor.id === cfg.editorId),
	});
	const { data: preferredFontFamily } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.diffFontFamily,
	});
	const { data: preferredFontSize } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.diffFontSize,
	});
	const { data: preferredTabSize } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.diffTabSize,
	});
	const { data: theme } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.theme,
	});
	const { mutate: openInProgram } = useOpenInProgram();
	const hunkMenuItems = useHunkMenuItems({ projectId });

	const diffSelection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionDiff(state, projectId, navigationIndex),
	);
	const diffSelectionFile =
		diffSelection !== null ? fileByHunkKey.get(hunkOperandIdentityKey(diffSelection)) : null;
	const selectedRange = diffSelection
		? (hunkByKey.get(hunkOperandIdentityKey(diffSelection))?.selectedLines ?? null)
		: null;

	const selectDiff = (selection: HunkOperand) => {
		dispatch(projectSlice.actions.selectDiff({ projectId, selection }));

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
		select: selectDiff,
		selection: diffSelection,
		selectSectionPredicate: (hunk) => {
			const k = hunkOperandIdentityKey(hunk);
			return hunkOperandIdentityKey(assert(assert(fileByHunkKey.get(k)).hunks[0]).operand) === k;
		},
		ref: selectionScopeRef,
		getKey: hunkOperandIdentityKey,
		operationSourcesForItem: (hunk) => [hunkOperand(hunk)],
	});

	useHotkeys([
		{
			hotkey: diffHotkeys.foldFile.hotkey,
			callback: () => !!diffSelectionFile && handleSetCollapsed(diffSelectionFile.item.id)(true),
			options: {
				enabled: !!diffSelectionFile && !collapsedItems.has(diffSelectionFile.item.id),
				conflictBehavior: "allow",
				target: selectionScopeRef,
				meta: diffHotkeys.foldFile.meta,
			},
		},
		{
			hotkey: diffHotkeys.unfoldFile.hotkey,
			callback: () => !!diffSelectionFile && handleSetCollapsed(diffSelectionFile.item.id)(false),
			options: {
				enabled: !!diffSelectionFile && collapsedItems.has(diffSelectionFile.item.id),
				conflictBehavior: "allow",
				target: selectionScopeRef,
				meta: diffHotkeys.unfoldFile.meta,
			},
		},
		{
			hotkey: diffHotkeys.openInEditor.hotkey,
			callback: () =>
				diffSelectionFile &&
				preferredEditor &&
				openInProgram({
					projectId,
					programId: preferredEditor.id,
					path: diffSelectionFile.change.path,
					lineNr: selectedRange?.range.start ?? null,
				}),
			options: {
				enabled: !!diffSelectionFile && !!preferredEditor,
				conflictBehavior: "allow",
				target: selectionScopeRef,
				meta: diffHotkeys.openInEditor.meta,
			},
		},
	]);

	const selectFileAtViewportTop = (scrollTop: number, viewer: CodeViewClass<undefined>): void => {
		if (didScrollToViaFileRef.current) {
			didScrollToViaFileRef.current = false;
			return;
		}

		const activeItem = viewer
			.getRenderedItems()
			// It can only be undefined if the item ID is invalid.
			.findLast((item) => assert(viewer.getTopForItem(item.id)) <= scrollTop);

		// This can happen on very fast scroll.
		if (activeItem === undefined) return;

		onViewerFileSelection(codeViewItemIdPath({ changesetKey, id: activeItem.id }));
	};

	// We currently only support selecting contiguous blocks.
	const handleLinesSelected = (sel: CodeViewLineSelection | null): void => {
		if (!sel) return void dispatch(projectSlice.actions.selectDiff({ projectId, selection: null }));

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
			projectSlice.actions.selectDiff({
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

	const getHunkOperandAtLine = ({
		itemId,
		lineNumber,
		side,
	}: DiffLineTarget): HunkOperand | null => {
		const file = fileByItemId.get(itemId);
		if (file?.patch?.type !== "Patch") return null;

		const selection = contiguousSelectionByLine({
			hunks: file.item.fileDiff.hunks,
			line: lineNumber,
			side,
		});
		if (!selection) return null;

		return {
			parent: {
				parent: fileParent,
				path: file.change.path,
			},
			...selection,
			isResultOfBinaryToTextConversion: file.patch.subject.isResultOfBinaryToTextConversion,
		};
	};

	const handleLineContextMenu = ({ event, ...target }: DiffLineContextMenuTarget): void => {
		const file = fileByItemId.get(target.itemId);
		if (!file) return;
		const operand = getHunkOperandAtLine(target);
		if (!operand) return;

		void showNativeContextMenu(
			event,
			hunkMenuItems({
				change: file.change,
				lineNumber: target.lineNumber,
				operand,
			}),
		);
	};

	useDiffLineContextMenu({
		viewerRef,
		onContextMenu: handleLineContextMenu,
	});

	const handleHunkPostRender = useDiffHunkDrag({
		projectId,
		getHunkOperand: getHunkOperandAtLine,
	});

	const handleSetCollapsed = (itemId: string) => (collapsed: boolean) => {
		const s = new Set(collapsedItems);

		if (collapsed) s.add(itemId);
		else s.delete(itemId);

		setCollapsedItems(s);
	};

	// We must change the version for updates to the collapsed property to be respected. The versions
	// should be as stable as possible, collapsed or not, for performance.
	const enhanceCollapsed = (item: CodeViewDiffItem): CodeViewDiffItem => ({
		...item,
		collapsed: true,
		// We always use versions.
		version: combineHashes(assert(item.version), 1),
	});

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
						collapsed={item.collapsed ?? false}
						setCollapsed={handleSetCollapsed(item.id)}
					/>
				);
			}}
			onScroll={selectFileAtViewportTop}
			className={styles.diffContents}
			items={
				collapsedItems.size === 0
					? items
					: items.map((item) => (collapsedItems.has(item.id) ? enhanceCollapsed(item) : item))
			}
			selectedLines={selectedRange}
			onSelectedLinesChange={handleLinesSelected}
			options={{
				diffStyle: diffStyle ?? diffDefaults.diffStyle,
				disableBackground: !(diffBackgrounds ?? diffDefaults.diffBackground),
				overflow: diffOverflow ?? diffDefaults.diffOverflow,
				themeType: theme ?? defaultSettings.theme,
				stickyHeaders: true,
				enableLineSelection: true,
				layout: {
					paddingTop: 0,
					// Match --panel-padding-block.
					paddingBottom: 12,
					gap: 10,
				},
				// This appears to validate before our custom header has been slotted, in which case - if
				// our metrics are correct - we should see deltas in multiples of our custom header height
				// as defined in the metrics. We'll see an additional set of logs if there are other issues
				// with our metrics.
				__devOnlyValidateItemHeights: false,
				onPostRender: handleHunkPostRender,
				itemMetrics: {
					// Computed custom header height.
					diffHeaderHeight: 38,
					// Default spacing plus our 1px border.
					paddingBottom: 9,
				},
				unsafeCSS: `
          :host {
            background-color: transparent;
          }

          [data-diffs-header="custom"] {
            background-color: var(--bg-1);
          }

          [data-code] {
            border-radius: 0 0 var(--radius-card) var(--radius-card);
          }

          [data-diff] {
            border-width: 0 1px 1px 1px;
            border-style: solid;
            border-color: var(--border-3);
            border-radius: 0 0 var(--radius-card) var(--radius-card);
          }

          [data-column-number] {
            --mix-selection-light: 0%;
            --mix-selection-dark: 0%;

            cursor: default;
          }
        `,
			}}
			style={{
				"--diffs-font-family": preferredFontFamily ?? defaultSettings.diffFontFamily,
				"--diffs-font-size": `${preferredFontSize ?? defaultSettings.diffFontSize}px`,
				"--diffs-tab-size": `${preferredTabSize ?? defaultSettings.diffTabSize}`,
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
	collapsed: boolean;
	setCollapsed: (collapsed: boolean) => void;
};

const DiffFileHeader: FC<DiffFileHeaderProps> = (p) => {
	const menuItems = useFileMenuItems({
		projectId: p.projectId,
		operand: p.operand,
		path: p.change.path,
		change: p.change,
	});

	const lastSepIdx = p.change.path.lastIndexOf("/");
	const directoryPath = lastSepIdx !== -1 ? p.change.path.slice(0, lastSepIdx) : null;
	const fileName = lastSepIdx !== -1 ? p.change.path.slice(lastSepIdx + 1) : p.change.path;

	const changeType = Match.value(p.item.fileDiff.type).pipe(
		Match.when("new", () => "Added"),
		Match.whenOr("change", "rename-changed", () => "Modified"),
		Match.when("rename-pure", () => "Renamed"),
		Match.when("deleted", () => "Deleted"),
		Match.exhaustive,
	);
	const collapseHotkey = p.collapsed ? diffHotkeys.unfoldFile : diffHotkeys.foldFile;
	const collapseLabel = collapseHotkey.meta.name;

	return (
		<OperationSourceC projectId={p.projectId} source={fileOperand(p.operand)} outline="inside">
			<header
				onContextMenu={(event) => {
					void showNativeContextMenu(event, menuItems);
				}}
				className={classes(styles.fileHeader, (p.collapsed || !p.hasDiff) && styles.lone)}
			>
				<Tooltip.Root>
					<Tooltip.Trigger
						aria-label={collapseLabel}
						aria-expanded={!p.collapsed}
						className={getButtonClassName({ size: "small", variant: "ghost", iconOnly: true })}
						onClick={() => p.setCollapsed(!p.collapsed)}
					>
						<Icon name={p.collapsed ? "chevron-right" : "chevron-down"} />
					</Tooltip.Trigger>
					<Tooltip.Portal>
						<Tooltip.Positioner sideOffset={4}>
							<Tooltip.Popup render={<TooltipPopup kbd={collapseHotkey.hotkey} />}>
								{collapseLabel}
							</Tooltip.Popup>
						</Tooltip.Positioner>
					</Tooltip.Portal>
				</Tooltip.Root>
				<h4 className={classes("text-13", styles.filePath)}>
					{fileName}
					{directoryPath !== null && <span className={styles.pathInit}>{directoryPath}</span>}
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
		Match.tags({
			Branch: ({ branchRef }) => (
				<SuspenseQuery
					{...branchDetailsQueryOptions({
						projectId,
						...branchDetailsParams(decodeBytes(branchRef)),
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
			File: ({ path }) => (
				<div className={styles.title}>
					<Icon name="file" />
					<h3 className={classes("text-15", "text-semibold")}>{path}</h3>
				</div>
			),
			Commit: ({ commitId }) => (
				<SuspenseQuery {...commitDetailsWithLineStatsQueryOptions({ projectId, commitId })}>
					{({ data: commitDetails }) => (
						<div className={styles.title}>
							<Icon name="commit" />
							<h3 className={classes(styles.titleContentWrapper, "text-15", "text-semibold")}>
								<span className={styles.titleContent}>
									{commitTitle(commitDetails.commit.message) ?? "(no message)"}
								</span>
								{commitDetails.commit.hasConflicts && (
									<Badge variant="danger" className={styles.commitConflictBadge}>
										Conflicted
									</Badge>
								)}

								{commitBody(commitDetails.commit.message) !== undefined && (
									<Tooltip.Root>
										<Tooltip.Trigger
											aria-controls={bodyId}
											aria-expanded={!bodyCollapsed}
											aria-label={bodyCollapsed ? "Expand commit body" : "Collapse commit body"}
											aria-pressed={!bodyCollapsed}
											className={classes(
												getButtonClassName({
													variant: bodyCollapsed ? "outline" : "gray",
													iconOnly: true,
													size: "small",
												}),
												styles.commitBodyToggle,
											)}
											onClick={() => onBodyCollapsedChange(!bodyCollapsed)}
										>
											<Icon name="kebab" />
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
							</h3>
						</div>
					)}
				</SuspenseQuery>
			),
		}),
		Match.orElseAbsurd,
	);

const FilesToggle: FC<
	Omit<ComponentProps<typeof Toggle>, "aria-label" | "pressed" | "onPressedChange">
> = (toggleProps) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const dispatch = useAppDispatch();
	const filesVisible = useAppSelector((state) =>
		projectSlice.selectors.selectFilesVisible(state, projectId),
	);

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				render={
					<Toggle
						{...toggleProps}
						aria-label="Toggle files"
						pressed={filesVisible}
						onPressedChange={() => dispatch(projectSlice.actions.toggleFiles({ projectId }))}
					/>
				}
			/>
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

const DiffOverflowToggle: FC<
	Omit<ComponentProps<typeof Toggle>, "aria-label" | "pressed" | "onPressedChange">
> = (toggleProps) => {
	const { data: diffOverflow } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.diffOverflow,
	});
	const { mutate: saveGUISettings } = useSaveGUISettings();

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				render={
					<Toggle
						{...toggleProps}
						aria-label="Toggle line wrapping"
						pressed={(diffOverflow ?? diffDefaults.diffOverflow) === "wrap"}
						onPressedChange={(pressed) =>
							saveGUISettings({ diffOverflow: pressed ? "wrap" : "scroll" })
						}
					/>
				}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>Toggle line wrapping</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const DiffBackgroundsToggle: FC<
	Omit<ComponentProps<typeof Toggle>, "aria-label" | "pressed" | "onPressedChange">
> = (toggleProps) => {
	const { data: diffBackgrounds } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.diffBackground,
	});
	const { mutate: saveGUISettings } = useSaveGUISettings();

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				render={
					<Toggle
						{...toggleProps}
						aria-label="Toggle diff backgrounds"
						pressed={diffBackgrounds ?? diffDefaults.diffBackground}
						onPressedChange={(enabled) => saveGUISettings({ diffBackground: enabled })}
					/>
				}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>Toggle diff backgrounds</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const DiffStyleToggleGroup: FC<
	Omit<
		ToggleGroup.Props<NonNullable<GUISettings["diffStyle"]>>,
		"aria-label" | "value" | "onValueChange"
	>
> = (toggleGroupProps) => {
	const { data: diffStyle } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.diffStyle,
	});
	const { mutate: saveGUISettings } = useSaveGUISettings();

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				render={
					<ToggleGroup
						{...toggleGroupProps}
						aria-label={diffHotkeys.toggleDiffStyle.meta.name}
						value={[diffStyle ?? diffDefaults.diffStyle]}
						onValueChange={(value: Array<NonNullable<GUISettings["diffStyle"]>>) => {
							const head = value[0];
							if (head === undefined) return;

							saveGUISettings({ diffStyle: head });
						}}
					/>
				}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup kbd={diffHotkeys.toggleDiffStyle.hotkey} />}>
						{diffHotkeys.toggleDiffStyle.meta.name}
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

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
			{body !== undefined && !bodyCollapsed && (
				<p id={bodyId} className={classes("text-monospace", "text-body", styles.commitMessageBody)}>
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
	filesItems: Array<FileRowItem>;
	onFileSelection: (selection: string) => void;
	outlineSelection: Operand;
	projectId: string;
}> = ({ changes, filesVisible, filesItems, onFileSelection, outlineSelection, projectId }) => {
	const selectionScopeRef = useRef<HTMLDivElement>(null);
	const viewerRef = useRef<CodeViewHandle<undefined>>(null);

	// On file selection we select the first hunk/block in that file and scroll to it, which triggers
	// CodeView's scroll handler, which in turn updates file selection again (as per usual scrolling
	// scenario). That latter file selection is based upon the first file visible in the viewport,
	// which may exclude trailing files collectively shorter than the scroll container.
	//
	// The callback doesn't provide any way of knowing what triggered the scroll, so we use this ref
	// to bypass that latter file selection. We could alternatively attempt to pad the scroll
	// container, but that comes with other complexities and tradeoffs.
	const didScrollToViaFileRef = useRef(false);

	const dispatch = useAppDispatch();
	const canShowFiles = useAppSelector((state) =>
		projectSlice.selectors.selectCanShowFiles(state, projectId),
	);
	const files = filesItems.map((item) => item.path);
	const filesNavigationIndex: NavigationIndex<string> = {
		items: files,
		indexByKey: buildIndexByKey(files, (path) => path),
	};
	const filesSelection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionFiles(state, projectId, filesNavigationIndex),
	);

	// At time of writing React Compiler cannot statically analyse that these are pure derivations of
	// the outline selection, even with the helpers inlined, hence manual memoisation.
	const changesetKey = useMemo(
		() =>
			Match.value(outlineSelection).pipe(
				Match.tags({
					Branch: ({ branchRef }) => decodeBytes(branchRef),
					File: ({ path }) => path,
					Commit: ({ commitId }) => commitId,
				}),
				Match.orElseAbsurd,
			),
		[outlineSelection],
	);
	const fileParent = useMemo(
		() =>
			Match.value(outlineSelection).pipe(
				Match.tags({
					Branch: ({ branchRef }) => branchFileParent({ branchRef }),
					File: ({ parent }) => parent,
					Commit: ({ commitId }) => commitFileParent({ commitId }),
				}),
				Match.orElseAbsurd,
			),
		[outlineSelection],
	);

	const treeChangeDiffs = useSuspenseQueries({
		queries: changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
		combine: (results) => results.map((result) => result.data),
	});

	const diffView = getDiffView({
		fileParent,
		changes,
		treeChangeDiffs,
		changesetKey,
	});

	const selectFileAndNavigateDiff = (selection: string) => {
		onFileSelection(selection);

		dispatch(
			projectSlice.actions.selectDiff({
				projectId,
				selection: diffView.fileByPath.get(selection)?.hunks[0]?.operand ?? null,
			}),
		);

		didScrollToViaFileRef.current = true;
		viewerRef.current?.scrollTo({
			type: "item",
			id: codeViewItemId({ changesetKey, path: selection }),
		});
	};

	const { data: diffSettings } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => ({
			diffBackground: cfg.diffBackground,
			diffOverflow: cfg.diffOverflow,
			diffStyle: cfg.diffStyle,
		}),
	});

	const { mutate: saveGUISettings } = useSaveGUISettings();

	const diffContentsEl = useRef<HTMLElement | null>(null);
	const [canUseSplitDiff, setCanUseSplitDiff] = useState<boolean | undefined>();

	useHotkeys([
		{
			hotkey: diffHotkeys.toggleDiffStyle.hotkey,
			callback: () =>
				saveGUISettings({
					diffStyle:
						(diffSettings?.diffStyle ?? diffDefaults.diffStyle) === "split" ? "unified" : "split",
				}),
			options: {
				conflictBehavior: "allow",
				enabled: canUseSplitDiff,
				meta: diffHotkeys.toggleDiffStyle.meta,
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
	const panelIds: Array<PanelId> = filesVisible ? ["files-panel", "diff-panel"] : ["diff-panel"];
	const diffLayout = useDefaultLayout({
		id: layoutId,
		panelIds,
	});

	return (
		<div className={styles.diffTab}>
			<div className={styles.actions}>
				{canShowFiles && <FilesToggle className={getButtonClassName({})}>Toggle files</FilesToggle>}

				<Toolbar.Root aria-label="Diff controls" className={styles.diffControls}>
					<ToggleGroupStyles>
						<Toolbar.Button
							render={
								<DiffOverflowToggle render={<ToggleStyles iconOnly />}>
									<Icon name="text-wrap" />
								</DiffOverflowToggle>
							}
						/>
						<Toolbar.Button
							render={
								<DiffBackgroundsToggle render={<ToggleStyles iconOnly />}>
									<Icon name="text-block" />
								</DiffBackgroundsToggle>
							}
						/>
					</ToggleGroupStyles>
					{canUseSplitDiff && (
						<DiffStyleToggleGroup render={<ToggleGroupStyles />}>
							<Toolbar.Button
								render={<Toggle render={<ToggleStyles />} />}
								value={"split" satisfies GUISettings["diffStyle"]}
							>
								Split
							</Toolbar.Button>
							<Toolbar.Button
								render={<Toggle render={<ToggleStyles />} />}
								value={"unified" satisfies GUISettings["diffStyle"]}
							>
								Unified
							</Toolbar.Button>
						</DiffStyleToggleGroup>
					)}
				</Toolbar.Root>
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
							id={"files-panel" satisfies PanelId}
							className={styles.panel}
							defaultSize={250}
							minSize={180}
							groupResizeBehavior="preserve-pixel-size"
						>
							<FilesTree
								data-selection-scope={"files" satisfies SelectionScope}
								className={classes(styles.diffFiles, uiStyles.scrollerWithSeparator)}
								onFileSelection={selectFileAndNavigateDiff}
								projectId={projectId}
								items={filesItems}
								selection={filesSelection}
								navigationIndex={filesNavigationIndex}
								fileParent={fileParent}
							/>
						</Panel>
						<Separator className={styles.resizeHandle} />
					</>
				)}

				<Panel id={"diff-panel" satisfies PanelId} minSize={300} className={styles.panel}>
					<div
						data-selection-scope={"diff" satisfies SelectionScope}
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
							diffBackgrounds={diffSettings?.diffBackground}
							diffOverflow={diffSettings?.diffOverflow}
							diffStyle={canUseSplitDiff ? diffSettings?.diffStyle : "unified"}
							selectionScopeRef={selectionScopeRef}
							viewerRef={viewerRef}
							didScrollToViaFileRef={didScrollToViaFileRef}
						/>
					</div>
				</Panel>
			</Group>
		</div>
	);
};

const PullRequestForm: FC<{
	projectId: string;
	sourceBranch: string;
	reviewId: number | null;
	title: string | null;
	body: string | null;
}> = ({ projectId, sourceBranch, reviewId, title, body }) => {
	const { isPending: isPublishReviewPending, mutate: publishReview } = usePublishReview();
	const { isPending: isUpdateReviewPending, mutate: updateReview } = useUpdateReview();
	const formRef = useRef<HTMLFormElement | null>(null);

	const remoteOrEmptyDocument = {
		title: title ?? "",
		body: body ?? "",
	};
	const { data: persistedDocument } = useSuspenseQuery(
		draftPRQueryOptions({ projectId, branchName: sourceBranch }),
	);
	const [localDocument, setLocalDocument] = useState({
		title: persistedDocument?.title ?? title ?? "",
		body: persistedDocument?.body ?? body ?? "",
		isDraft: persistedDocument?.isDraft ?? false,
	});
	const { mutate: persistDraftPR } = usePersistDraftPR();

	const isNew = reviewId === null;
	const isAnyPending = isPublishReviewPending || isUpdateReviewPending;
	const hasChanges =
		localDocument.title !== remoteOrEmptyDocument.title ||
		localDocument.body !== remoteOrEmptyDocument.body ||
		(isNew && localDocument.isDraft);

	const handleBlur = () => {
		persistDraftPR({
			projectId,
			branchName: sourceBranch,
			draft: localDocument,
		});
	};

	const handleReset = () => {
		const resetDocument = { ...remoteOrEmptyDocument, isDraft: false };
		setLocalDocument(resetDocument);
		persistDraftPR({
			projectId,
			branchName: sourceBranch,
			draft: resetDocument,
		});
	};

	const handleSubmit: SubmitEventHandler<HTMLFormElement> = (evt) => {
		evt.preventDefault();
		if (isAnyPending || localDocument.title.trim() === "") return;

		if (reviewId === null) {
			publishReview({
				projectId,
				params: {
					title: localDocument.title,
					body: localDocument.body,
					draft: localDocument.isDraft,
					localBranch: sourceBranch,
					sourceBranch,
				},
			});
		} else {
			updateReview({
				projectId,
				reviewId,
				title: localDocument.title,
				body: localDocument.body,
				state: null,
				targetBase: null,
			});
		}
	};

	useHotkey(pullRequestHotkeys.update.hotkey, () => formRef.current?.requestSubmit(), {
		conflictBehavior: "allow",
		enabled: !isAnyPending && hasChanges,
		target: formRef,
	});

	return (
		// oxlint-disable-next-line jsx-a11y/no-noninteractive-element-interactions -- Used for persistence, not UI per se.
		<form ref={formRef} className={styles.prForm} onBlur={handleBlur} onSubmit={handleSubmit}>
			<Field.Root render={<FieldRootStyles />}>
				<Field.Label render={<FieldLabelStyles />}>Title</Field.Label>
				<Field.Control
					render={<FieldControlStyles />}
					className="text-15 text-semibold"
					name="title"
					onChange={(evt) => setLocalDocument({ ...localDocument, title: evt.currentTarget.value })}
					placeholder="Title"
					required
					value={localDocument.title}
				/>
			</Field.Root>

			<Field.Root render={<FieldRootStyles />}>
				<Field.Label render={<FieldLabelStyles />}>Description</Field.Label>
				<Field.Control
					render={<FieldTextareaStyles />}
					className="text-14 text-body text-monospace"
					name="body"
					onChange={(evt) => setLocalDocument({ ...localDocument, body: evt.currentTarget.value })}
					placeholder="Description"
					value={localDocument.body}
				/>
			</Field.Root>

			{isNew && (
				<Field.Root render={<FieldRootStyles />}>
					<Field.Label render={<FieldLabelStyles />}>Draft</Field.Label>
					<Checkbox
						checked={localDocument.isDraft}
						name="isDraft"
						onCheckedChange={(isDraft) => setLocalDocument({ ...localDocument, isDraft })}
					/>
				</Field.Root>
			)}

			<div className={styles.prFormActions}>
				<button
					className={getButtonClassName({})}
					disabled={isAnyPending || !hasChanges}
					onClick={handleReset}
					type="button"
				>
					Reset
				</button>

				<button
					className={getButtonClassName({ variant: "pop" })}
					disabled={isAnyPending || !hasChanges}
					type="submit"
				>
					{isAnyPending && <Icon name="spinner" />}
					{isNew ? "Submit" : "Update"}
				</button>
			</div>
		</form>
	);
};

const PullRequestPrimaryAction: FC<{
	projectId: string;
	reviewId: number;
	isDraft: boolean;
}> = ({ projectId, reviewId, isDraft }) => {
	const { data: mergeStatus } = useQuery({
		...getReviewMergeStatusQueryOptions({ projectId, reviewId }),
		// Minimise API calls.
		enabled: !isDraft,
	});

	const { isPending: isUpdateReviewPending, mutate: updateReview } = useUpdateReview();
	const { isPending: isMergeReviewPending, mutate: mergeReview } = useMergeReview();
	const { isPending: isSetReviewDraftinessPending, mutate: setReviewDraftiness } =
		useSetReviewDraftiness();
	const { isPending: isSetReviewAutoMergePending, mutate: setReviewAutoMerge } =
		useSetReviewAutoMerge();

	const isAnyPending =
		isUpdateReviewPending ||
		isMergeReviewPending ||
		isSetReviewDraftinessPending ||
		isSetReviewAutoMergePending;

	return (
		<div className={styles.prActions}>
			<button
				className={getButtonClassName({ variant: !isDraft ? "outline" : "pop" })}
				disabled={isAnyPending}
				onClick={() => setReviewDraftiness({ projectId, reviewId, draft: !isDraft })}
				type="button"
			>
				{isSetReviewDraftinessPending && <Icon name="spinner" />}
				{isDraft ? "Mark as Ready" : "Convert to draft"}
			</button>

			<button
				className={getButtonClassName({ variant: "danger" })}
				disabled={isAnyPending}
				onClick={() =>
					updateReview({
						projectId,
						reviewId,
						state: "closed",
						title: null,
						body: null,
						targetBase: null,
					})
				}
				type="button"
			>
				{isUpdateReviewPending && <Icon name="spinner" />}
				Close
			</button>

			{!isDraft && (
				<>
					<button
						className={getButtonClassName({ variant: "outline" })}
						// Currently missing automerge state from SDK.
						disabled
						onClick={() => setReviewAutoMerge({ projectId, reviewId, enable: true })}
						type="button"
					>
						{isSetReviewAutoMergePending && <Icon name="spinner" />}
						Enable auto-merge
					</button>

					<button
						className={getButtonClassName({ variant: "pop" })}
						disabled={isAnyPending || mergeStatus?.isMergeable !== true}
						onClick={() => mergeReview({ projectId, reviewId, mergeMethod: null })}
						type="button"
					>
						{isMergeReviewPending && <Icon name="spinner" />}
						Merge
					</button>
				</>
			)}
		</div>
	);
};

const Check: FC<{
	title: string;
	icon: IconName;
	iconColor: string;
	url: string;
}> = (p) => {
	const handleOpen =
		(url: string) =>
		async (evt: MouseEvent<HTMLAnchorElement>): Promise<void> => {
			evt.preventDefault();

			await window.lite.openInWebBrowser(url);
		};

	return (
		<a
			href={p.url}
			onClick={(evt) => void handleOpen(p.url)(evt)}
			className={classes("text-13", styles.check)}
		>
			<Icon name={p.icon} style={{ color: p.iconColor }} />
			{p.title}
		</a>
	);
};

const Checks: FC<{ checks: Array<CiCheck>; aggregate: AggregateCIChecks }> = (p) => {
	const [summary, summaryIcon, summaryIconColor] = Match.value(p.aggregate.status).pipe(
		Match.withReturnType<[string, IconName, string]>(),
		Match.when("success", () => ["All passed", "checklist", "var(--scale-safe-50)"]),
		Match.when("failure", () => ["Failed", "checklist-remove", "var(--scale-danger-50)"]),
		Match.when("cancelled", () => ["Some cancelled", "checklist-remove", "var(--scale-danger-50)"]),
		Match.when("action_required", () => ["Action required", "warning", "var(--scale-warn-50)"]),
		Match.when("in_progress", () => [
			"In progress",
			"spinner",
			p.aggregate.failure.length > 0
				? "var(--scale-danger-50)"
				: p.aggregate.actionRequired.length > 0
					? "var(--scale-warn-50)"
					: "grey",
		]),
		Match.when("unknown", () => ["Unknown", "warning", "var(--scale-purple-50)"]),
		Match.exhaustive,
	);

	return (
		<div className={styles.checks}>
			<h4 className={classes("text-14", styles.checkHeading)}>Checks</h4>

			<div className={classes("text-14", styles.checkSummary)}>
				<Icon name={summaryIcon} style={{ color: summaryIconColor }} />
				{summary}
			</div>

			{(p.aggregate.failure.length > 0 || p.aggregate.actionRequired.length > 0) && (
				<div className={styles.checkItems}>
					<h5 className={classes("text-13", styles.checkJobsHeading)}>Failed jobs</h5>

					{p.aggregate.failure.map((check) => (
						<Check
							key={check.id}
							title={check.name}
							icon="cross-circle"
							iconColor="var(--scale-danger-60)"
							url={check.htmlUrl}
						/>
					))}

					{p.aggregate.actionRequired.map((check) => (
						<Check
							key={check.id}
							title={check.name}
							icon="warning"
							iconColor="var(--scale-warn-60)"
							url={check.htmlUrl}
						/>
					))}
				</div>
			)}
		</div>
	);
};

export const Details: FC<
	{
		outlineSelection: Operand | null;
	} & ComponentProps<"div">
> = ({ outlineSelection, ...restProps }) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const headInfoIndex = headInfo ? getHeadInfoIndex(headInfo) : null;
	const dispatch = useAppDispatch();
	const detailsFullWindow = useAppSelector(interfaceSlice.selectors.selectDetailsFullWindow);
	const filesVisibleState = useAppSelector((state) =>
		projectSlice.selectors.selectFilesVisible(state, projectId),
	);
	const canShowFiles = useAppSelector((state) =>
		projectSlice.selectors.selectCanShowFiles(state, projectId),
	);
	const filesVisible = canShowFiles && filesVisibleState;
	const [commitBodyCollapsed, setCommitBodyCollapsed] = useState(true);
	const [branchTab, setBranchTab] = useState<BranchTab>("diff");
	const commitBodyId = useId();

	const selectFile = (selection: string) => {
		dispatch(projectSlice.actions.selectFiles({ projectId, selection }));
	};

	if (!outlineSelection) return;

	return (
		<div {...restProps} className={classes(restProps.className, styles.container)}>
			<div className={styles.headerWrap}>
				<div className={styles.titleRow}>
					{detailsFullWindow && <TopLeftControls />}

					<Title
						bodyCollapsed={commitBodyCollapsed}
						bodyId={commitBodyId}
						onBodyCollapsedChange={setCommitBodyCollapsed}
						projectId={projectId}
						selection={outlineSelection}
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

						{!!forgeInfo?.capabilities.prService && (
							<Suspense>
								<SuspenseQuery
									{...listReviewsQueryOptions({
										projectId,
										cacheConfig: "noCache",
									})}
								>
									{({ data }) => {
										const review = data.reviewsBySourceBranch.get(
											// https://linear.app/gitbutler/issue/GB-1226/unify-branch-identifiers
											decodeBytes(outlineSelection.branchRef).replace(/^refs\/heads\//, ""),
										);
										if (!review) return null;

										return (
											<div className={styles.tabsRowRight}>
												<PullRequestPrimaryAction
													projectId={projectId}
													reviewId={review.number}
													isDraft={review.draft}
												/>
											</div>
										);
									}}
								</SuspenseQuery>
							</Suspense>
						)}
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
						filesItems: Array<FileRowItem>;
						outlineSelection?: Operand;
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
						Match.tags({
							Commit: (commit) => (
								<SuspenseQuery
									{...commitDetailsWithLineStatsQueryOptions({
										projectId,
										commitId: commit.commitId,
									})}
								>
									{({ data: commitDetails }) =>
										renderDiff({
											changes: commitDetails.changes,
											filesItems: getCommitFileRowItems({ commitDetails }),
										})
									}
								</SuspenseQuery>
							),
							File: (file) => {
								if (file.parent._tag !== "UncommittedChanges") return null;

								return (
									<SuspenseQuery {...changesInWorktreeQueryOptions(projectId)}>
										{({ data: worktreeChanges }) => {
											const filesItems = getChangesFileRowItems(worktreeChanges).filter(
												(item) => item.path === file.path,
											);
											const changes = filesItems.flatMap((item) =>
												item._tag === "Change" ? [item.change] : [],
											);

											if (changes.length === 0) return null;

											return renderDiff({
												changes,
												filesItems,
											});
										}}
									</SuspenseQuery>
								);
							},
							Branch: ({ branchRef }) => {
								// Use push status of segment, not branch details; something about remote
								// tracking refs.
								const branchCtx = headInfoIndex?.branchContextByRefBytes(branchRef);
								const sourceBranch = branchCtx?.segment.refName?.displayName;
								const parentSegment = branchCtx?.stack.segments[branchCtx.segmentIndex + 1];
								const targetBranch =
									!parentSegment || parentSegment.pushStatus === "integrated"
										? headInfo?.target?.remoteTrackingRef.displayName
										: parentSegment.pushStatus === "completelyUnpushed"
											? undefined
											: parentSegment.refName?.displayName;

								return branchTab === "pr" ? (
									<div className={styles.prTab}>
										{!forgeInfo?.capabilities.prService ? (
											<p className="text-13">No valid forge.</p>
										) : targetBranch === undefined ? (
											<p className="text-13">No remote target branch.</p>
										) : sourceBranch === undefined ? (
											<p className="text-13">No source branch.</p>
										) : branchCtx?.segment.pushStatus === "completelyUnpushed" ? (
											<p className="text-13">Branch must be pushed to create PR.</p>
										) : (
											<SuspenseQuery
												{...listReviewsQueryOptions({
													projectId,
													cacheConfig: "noCache",
												})}
											>
												{({ data }) => {
													const review = data.reviewsBySourceBranch.get(sourceBranch);

													return !review ? (
														<PullRequestForm
															key={sourceBranch}
															body={null}
															projectId={projectId}
															reviewId={null}
															sourceBranch={sourceBranch}
															title={null}
														/>
													) : (
														<>
															<PullRequestForm
																key={review.number}
																body={review.body}
																projectId={projectId}
																reviewId={review.number}
																sourceBranch={sourceBranch}
																title={review.title}
															/>

															{forgeInfo.capabilities.checks && (
																<SuspenseQuery
																	{...listCIChecksQueryOptions({
																		projectId,
																		reference: sourceBranch,
																		polling: "priority",
																	})}
																>
																	{({ data: { data: checks, aggregate } }) =>
																		aggregate && <Checks checks={checks} aggregate={aggregate} />
																	}
																</SuspenseQuery>
															)}
														</>
													);
												}}
											</SuspenseQuery>
										)}
									</div>
								) : (
									<SuspenseQuery
										{...branchDiffQueryOptions({ projectId, branch: decodeBytes(branchRef) })}
									>
										{({ data: branchDiff }) =>
											renderDiff({
												changes: branchDiff.changes,
												filesItems: getBranchFileRowItems({ branchDiff }),
											})
										}
									</SuspenseQuery>
								);
							},
						}),
						Match.orElse(() => null),
					);
				})()}
			</Suspense>
		</div>
	);
};
