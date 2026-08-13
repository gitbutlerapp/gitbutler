import { ResizeHandle } from "#ui/components/ResizeHandle.tsx";
import { Scroller } from "#ui/components/Scroller.tsx";
import { SuspenseQuery } from "@suspensive/react-query";
import {
	useOpenInProgram,
	useResolveCommitConflictHunks,
	useSaveGUISettings,
} from "#ui/api/mutations.ts";
import {
	branchDiffQueryOptions,
	changesInWorktreeQueryOptions,
	commentsQueryOptions,
	commitConflictsQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	forgeInfoOptions,
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	listEditorsQueryOptions,
	listReviewsQueryOptions,
	treeChangeDiffsQueryOptions,
} from "#ui/api/queries.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { branchDetailsParams } from "#ui/branch.ts";
import { commitBody, commitTitle, shortCommitId } from "#ui/commit.ts";
import {
	branchFileParent,
	branchIdentityKey,
	commitFileParent,
	type FileOperand,
	fileOperand,
	hunkOperand,
	type FileParent,
	type HunkOperand,
	type Operand,
	weakCommitIdentityKey,
	weakFileParentIdentityKey,
} from "#ui/operands.ts";
import type { BranchTab } from "#ui/projects/project.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import { Badge } from "#ui/components/Badge.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { SelectionScopeKbd } from "#ui/components/SelectionScopeKbd.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { PullRequestComments } from "#ui/routes/project/$id/workspace/PullRequestComments.tsx";
import { PullRequestPanel } from "#ui/routes/project/$id/workspace/PullRequestPanel.tsx";
import {
	PullRequestDescription,
	PullRequestForm,
	PullRequestPrimaryAction,
} from "#ui/routes/project/$id/workspace/PullRequestTab.tsx";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { Toggle, ToggleGroup, Toolbar, Tooltip } from "@base-ui/react";
import type { CommitDetails, ConflictedFile, ManualConflict, TreeChange } from "@gitbutler/but-sdk";
import {
	type CodeViewDiffItem,
	type CodeView as CodeViewClass,
	type CodeViewLineSelection,
	type GetHoveredLineResult,
	type DiffLineAnnotation,
	isDiffAnnotation,
} from "@pierre/diffs";
import { CodeView, type CodeViewHandle } from "@pierre/diffs/react";
import {
	keepPreviousData,
	useQuery,
	useSuspenseQueries,
	useSuspenseQuery,
} from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Match } from "effect";
import {
	type ComponentProps,
	type FC,
	type ReactNode,
	type RefObject,
	Suspense,
	useId,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { Group, Panel, useDefaultLayout } from "react-resizable-panels";
import styles from "./Details.module.css";
import { diffHotkeys, workspaceHotkeys } from "#ui/hotkeys.ts";
import { useHotkeys } from "@tanstack/react-hotkeys";
import {
	type SelectionScope,
	useAutofocusSelectionScope,
	useNavigationIndexHotkeys,
} from "#ui/selection-scopes.ts";
import { ChangeStats } from "#ui/routes/project/$id/workspace/ChangeStats.tsx";
import { ChangesHeaderRow } from "#ui/routes/project/$id/workspace/ChangesHeaderRow.tsx";
import { getLineStats } from "#ui/routes/project/$id/workspace/lineStats.ts";
import { FilesTree } from "#ui/routes/project/$id/workspace/FilesTree.tsx";
import { TopLeftControls } from "#ui/routes/project/$id/workspace/TopLeftControls.tsx";
import {
	changeFileRowItem,
	conflictFileRowItem,
	getChangesFileRowItems,
	pathMatchesFilter,
	type FileRowItem,
} from "./file-row.ts";
import {
	buildFileTreeRows,
	fileTreeNavigationIndex,
	selectedFilePath,
	type FileDisplayMode,
	type FileTreeRow,
} from "./file-tree.ts";
import { useFileDisplayMode } from "./useFileDisplayMode.ts";
import { FileFilterRow } from "./FileFilterRow.tsx";
import { useFileFilter } from "./useFileFilter.ts";
import { contiguousSelectionByLine, wholeHunkSelectionByLine } from "#ui/hunk.ts";
import { showNativeContextMenu, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import { useFileMenuItems } from "#ui/routes/project/$id/workspace/useFileMenuItems.ts";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import type { GUISettings } from "#electron/settings.ts";
import { defaultSettings } from "#ui/settings.ts";
import type { IconName } from "#ui/components/iconNames.ts";
import { combineHashes, hash } from "#ui/hash.ts";
import { compareFilePaths } from "#ui/file-order.ts";
import { assert } from "#ui/assert.ts";
import {
	type DiffLineContextMenuTarget,
	useDiffLineContextMenu,
} from "./diff-line-context-menu.ts";
import { diffGutterUnsafeCSS, useDiffGutterCheckboxes } from "./diff-gutter.ts";
import { useDiffHunkDrag } from "./diff-hunk-drag.ts";
import type { DiffLineTarget } from "./diff-line-target.ts";
import { useHunkMenuItems } from "./useHunkMenuItems.ts";
import { ChangeTypeBadge } from "./ChangeTypeBadge.tsx";
import { AnnotationCard } from "#ui/routes/project/$id/workspace/AnnotationCard.tsx";
import { ConflictBar } from "#ui/routes/project/$id/workspace/ConflictBar.tsx";
import {
	annotationSideToDiffSide,
	annotationsByPathForScope,
	type LocalAnnotationsByPath,
	useCommentCreate,
} from "#ui/annotation.ts";
import { FileIcon } from "#ui/components/FileIcon.tsx";
import {
	type Annotation,
	codeViewItemMetrics,
	codeViewLayout,
	type DiffView,
	getDiffView,
	hunkOperandIdentityKey,
	prepareDiffFiles,
	withoutFoldedHunks,
} from "./diff-view.ts";
import { DiffMinimap } from "./DiffMinimap.tsx";
import { getMinimapFiles, measureWrapColumns, type MinimapSelection } from "./diff-minimap.ts";
import { checkedRange, navigationIndexRange } from "#ui/checking.ts";
import {
	type ReviewedFileVersions,
	reviewedFilesQueryOptions,
	type SetFilesReviewedInput,
	useSetFilesReviewed,
} from "#ui/reviewed-files.ts";

export type DiffViewerHandle = CodeViewHandle<Annotation>;

// This must be unique as to not collide with other IDs, and stable because it's
// stored in local storage.
type PanelId = "files-panel" | "diff-panel";

const EMPTY_ANNOTATIONS_BY_PATH: LocalAnnotationsByPath = new Map();
const EMPTY_CONFLICTS: Array<ConflictedFile> = [];
const EMPTY_MANUAL: Array<ManualConflict> = [];

const isInteractiveElement = (target: EventTarget): boolean =>
	target instanceof Element &&
	target.matches(
		'a, button, input, select, textarea, [contenteditable]:not([contenteditable="false"])',
	);

const getCommitFileRowItems = ({
	commitDetails,
	manual = EMPTY_MANUAL,
}: {
	commitDetails: CommitDetails;
	/**
	 * Conflicted files the resolve API cannot address. They have no diff to
	 * show, which is exactly what a conflict row is — so they keep the commit
	 * visibly conflicted while pointing at the files that need edit mode.
	 */
	manual?: Array<ManualConflict>;
}): Array<FileRowItem> => {
	const conflictedPaths = globalThis.Array.from(
		new Set([
			...(commitDetails.conflictEntries
				? [
						...commitDetails.conflictEntries.ancestorEntries,
						...commitDetails.conflictEntries.ourEntries,
						...commitDetails.conflictEntries.theirEntries,
					]
				: []),
			...manual.map((file) => file.path),
		]),
	).toSorted((a, b) => a.localeCompare(b));
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

const withAnnotations = (
	diffView: DiffView,
	annotationsByPath: LocalAnnotationsByPath,
): DiffView => ({
	...diffView,
	items: diffView.items.map((item) => {
		const file = diffView.fileByItemId.get(item.id);
		if (!file) throw new Error("Diff view file not found by ID");

		const persistedAnnotations = annotationsByPath.get(file.operand.path);
		if (!persistedAnnotations || persistedAnnotations.length === 0) return item;

		const annotations: Array<DiffLineAnnotation<Annotation>> = persistedAnnotations.map(
			({ id, lineNumber, side }) => ({
				lineNumber,
				side,
				metadata: { _tag: "local", id },
			}),
		);

		// Annotations move when their backend anchor drifts, so the version must cover their
		// positions and identities, not just their count.
		const annoHash = hash(
			persistedAnnotations.map((a) => `${a.id}:${a.side}:${a.lineNumber}`).join(),
		);

		const version = item.version;
		if (version === undefined) throw new Error("Diff view item missing base version");

		return {
			...item,
			version: combineHashes(version, annoHash),
			annotations,
		};
	}),
});

const DiffContents: FC<{
	activeFileItemId: string | null;
	selectionScopeRef: RefObject<HTMLDivElement | null>;
	onViewerFileSelection: (path: string) => void;
	fileParent: FileParent;
	projectId: string;
	diffView: DiffView;
	annotationsByPath: LocalAnnotationsByPath;
	diffBackgrounds?: GUISettings["diffBackground"];
	diffOverflow?: GUISettings["diffOverflow"];
	diffStyle?: GUISettings["diffStyle"];
	reviewedFiles: ReviewedFileVersions;
	manualCollapseByItem: Map<string, boolean>;
	setManualCollapse: (itemId: string, collapsed: boolean | undefined) => void;
	setFilesReviewed: (input: SetFilesReviewedInput) => void;
	viewerRef: RefObject<CodeViewHandle<Annotation> | null>;
	didScrollToViaFileRef: RefObject<boolean>;
}> = ({
	activeFileItemId,
	selectionScopeRef,
	onViewerFileSelection,
	fileParent,
	projectId,
	diffView: { items, navigationIndex, hunkByKey, fileByItemId },
	annotationsByPath,
	diffBackgrounds,
	diffOverflow,
	diffStyle,
	reviewedFiles,
	manualCollapseByItem,
	setManualCollapse,
	setFilesReviewed,
	viewerRef,
	didScrollToViaFileRef,
}) => {
	const newFocusableAnnotationIdRef = useRef<string | null>(null);
	const dispatch = useAppDispatch();
	const { mutate: createComment } = useCommentCreate();
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: settings } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => ({
			editor: editors?.find((editor) => editor.id === cfg.editorId),
			diffFontFamily: cfg.diffFontFamily,
			diffFontSize: cfg.diffFontSize,
			diffLigatures: cfg.diffLigatures,
			diffTabSize: cfg.diffTabSize,
			lineDiffType: cfg.lineDiffType,
			theme: cfg.theme,
		}),
	});
	const { mutate: openInProgram } = useOpenInProgram();
	const hunkMenuItems = useHunkMenuItems({ projectId });
	const store = useAppStore();
	const hunkCheckRangeAnchor = useRef<string>(null);
	const hunkCheckRangeEnd = useRef<string>(null);

	const collapsedItems: Set<string> = new Set(
		items.flatMap((item) => {
			const manuallyCollapsed = manualCollapseByItem.get(item.id);
			if (manuallyCollapsed !== undefined) return manuallyCollapsed ? item.id : [];

			const file = fileByItemId.get(item.id);
			if (!file) return [];

			const {
				change: { path },
				item: { version },
			} = file;
			if (version === undefined) return [];

			const reviewedLatestVersion = reviewedFiles.get(path)?.has(version);
			return reviewedLatestVersion ? item.id : [];
		}),
	);
	const visibleNavigationIndex = withoutFoldedHunks(navigationIndex, hunkByKey, collapsedItems);

	const diffSelection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionDiff(state, projectId, visibleNavigationIndex),
	);
	const hasStoredDiffSelection = useAppSelector(
		(state) => projectSlice.selectors.selectStoredDiffSelection(state, projectId) !== null,
	);
	const canCheckHunks = useAppSelector((state) =>
		projectSlice.selectors.selectCanCheckHunks(state, projectId, fileParent),
	);
	const diffSelectionHunk =
		diffSelection !== null ? hunkByKey.get(hunkOperandIdentityKey(diffSelection)) : null;
	const selectedRange = diffSelection
		? (hunkByKey.get(hunkOperandIdentityKey(diffSelection))?.selectedLines ?? null)
		: null;
	// A primitive, null while the selection sits on a visible hunk, so the item
	// list and header closures below only pick up new identities when a folded
	// file gains or loses the selection — not on every j/k move.
	const selectedFoldedFileId =
		diffSelectionHunk != null && collapsedItems.has(diffSelectionHunk.file.item.id)
			? diffSelectionHunk.file.item.id
			: null;

	useLayoutEffect(() => {
		// The resolved hunk can belong to another file when the active file is hunkless.
		const itemId = activeFileItemId ?? diffSelectionHunk?.file.item.id;
		if (itemId === undefined) return;

		viewerRef.current?.scrollTo({
			type: "item",
			id: itemId,
			align: "start",
			behavior: "instant",
		});
		// oxlint-disable-next-line react-hooks/exhaustive-deps react-hooks-js/exhaustive-deps -- Sync scroll only on mount, otherwise use events.
	}, []);

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
		navigationIndex: visibleNavigationIndex,
		projectId,
		group: "Diff",
		select: selectDiff,
		selection: diffSelection,
		selectSectionPredicate: (hunk) => {
			const k = hunkOperandIdentityKey(hunk);
			return hunkOperandIdentityKey(assert(assert(hunkByKey.get(k)?.file.hunks[0])).operand) === k;
		},
		ref: selectionScopeRef,
		getKey: hunkOperandIdentityKey,
		operationSourcesForItem: (hunk) => {
			const source = hunkOperand(hunk);
			const state = store.getState();
			return projectSlice.selectors.selectOperandChecked(state, projectId, source)
				? projectSlice.selectors.selectCheckedOperands(state, projectId)
				: [source];
		},
	});

	function toggleSelectedHunkChecked(event: KeyboardEvent): void {
		if (diffSelection === null || event.composedPath().some(isInteractiveElement)) return;

		event.preventDefault();
		event.stopPropagation();
		checkHunk({ operand: diffSelection, shiftKey: event.shiftKey });
	}

	useHotkeys([
		{
			hotkey: diffHotkeys.checkHunk.hotkey,
			callback: toggleSelectedHunkChecked,
			options: {
				conflictBehavior: "allow",
				enabled: diffSelection !== null && canCheckHunks,
				preventDefault: false,
				stopPropagation: false,
				target: selectionScopeRef,
				meta: diffHotkeys.checkHunk.meta,
			},
		},
		{
			hotkey: "Shift+Space",
			callback: toggleSelectedHunkChecked,
			options: {
				conflictBehavior: "allow",
				enabled: diffSelection !== null && canCheckHunks,
				preventDefault: false,
				stopPropagation: false,
				target: selectionScopeRef,
			},
		},
		{
			hotkey: diffHotkeys.toggleFoldFile.hotkey,
			callback: () => {
				if (!diffSelectionHunk) return;

				handleSetCollapsed(diffSelectionHunk.file.item.id)(
					!collapsedItems.has(diffSelectionHunk.file.item.id),
				);
			},
			options: {
				// A stored selection, not the resolver's first-hunk fallback: after
				// scrolling with nothing selected, folding the fallback would fold a
				// file far off-screen. j/k (which stores a selection) is the way in.
				enabled: hasStoredDiffSelection && !!diffSelectionHunk,
				conflictBehavior: "allow",
				target: selectionScopeRef,
				meta: diffHotkeys.toggleFoldFile.meta,
			},
		},
		{
			hotkey: diffHotkeys.toggleReviewedFile.hotkey,
			callback: () => {
				if (!diffSelectionHunk) return;

				const { id, version } = diffSelectionHunk.file.item;
				if (version === undefined) throw new Error("Diff view item missing version");

				const { path } = diffSelectionHunk.file.change;
				handleSetReviewed(id, path, version)(!reviewedFiles.get(path)?.has(version));
			},
			options: {
				enabled: hasStoredDiffSelection && !!diffSelectionHunk,
				conflictBehavior: "allow",
				target: selectionScopeRef,
			},
		},
		{
			hotkey: diffHotkeys.openInEditor.hotkey,
			callback: () =>
				diffSelectionHunk &&
				settings?.editor &&
				openInProgram({
					projectId,
					programId: settings.editor.id,
					path: diffSelectionHunk.file.change.path,
					lineNr: selectedRange?.range.start ?? null,
				}),
			options: {
				enabled: !!diffSelectionHunk && !!settings?.editor,
				conflictBehavior: "allow",
				target: selectionScopeRef,
				meta: diffHotkeys.openInEditor.meta,
			},
		},
	]);

	const selectFileAtViewportTop = (scrollTop: number, viewer: CodeViewClass<Annotation>): void => {
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

		const file = fileByItemId.get(activeItem.id);
		if (!file) return;

		onViewerFileSelection(file.operand.path);
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
		lineType,
	}: DiffLineTarget): HunkOperand | null => {
		const file = fileByItemId.get(itemId);
		if (file?.patch?.type !== "Patch") return null;

		const query = { hunks: file.item.fileDiff.hunks, line: lineNumber, side };
		const selection =
			lineType === "context" ? wholeHunkSelectionByLine(query) : contiguousSelectionByLine(query);
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

	const hunkRangeResolver = navigationIndexRange<HunkOperand, string>({
		navigationIndex: visibleNavigationIndex,
		getKey: (key) => key,
		filterMap: hunkOperandIdentityKey,
	});
	const getCheckedHunkRange = checkedRange(hunkRangeResolver);

	function checkHunk({ operand, shiftKey }: { operand: HunkOperand; shiftKey: boolean }): void {
		const checkedHunks = projectSlice.selectors
			.selectCheckedOperands(store.getState(), projectId)
			.filter((operand) => operand._tag === "Hunk");
		const checkedHunksByKey = new Map(
			checkedHunks.map((operand) => [hunkOperandIdentityKey(operand), operand]),
		);
		const previous = new Set(checkedHunksByKey.keys());
		const nextHunkRange = getCheckedHunkRange({
			checked: previous,
			rangeAnchor: hunkCheckRangeAnchor.current,
			rangeEnd: hunkCheckRangeEnd.current,
		})({
			item: hunkOperandIdentityKey(operand),
			shiftKey,
		});

		hunkCheckRangeAnchor.current = nextHunkRange.rangeAnchor;
		hunkCheckRangeEnd.current = nextHunkRange.rangeEnd;

		dispatch(
			projectSlice.actions.checkOperands({
				projectId,
				operands: Array.from(nextHunkRange.checked.difference(previous)).flatMap((key) => {
					const hunk = hunkByKey.get(key);
					return hunk ? [hunkOperand(hunk.operand)] : [];
				}),
				checked: true,
			}),
		);
		dispatch(
			projectSlice.actions.checkOperands({
				projectId,
				operands: Array.from(previous.difference(nextHunkRange.checked)).flatMap((key) => {
					const hunk = checkedHunksByKey.get(key);
					return hunk ? [hunk] : [];
				}),
				checked: false,
			}),
		);
	}

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

	const handleHunkPostRender = useDiffHunkDrag<Annotation>({
		projectId,
		getHunkOperand: getHunkOperandAtLine,
	});
	const { onPostRender: handleDiffPostRender, portals: diffGutterPortals } =
		useDiffGutterCheckboxes(handleHunkPostRender, getHunkOperandAtLine, projectId, checkHunk);

	const handOffCollapsedSelection = (itemId: string): void => {
		// Folding hides the selected hunk's lines; hand the selection to the
		// file's first hunk, which stands in for the folded file, and keep the
		// header in view. The stored selection is read off the store rather than
		// captured, so this callback's identity does not churn with j/k moves.
		const stored = projectSlice.selectors.selectStoredDiffSelection(store.getState(), projectId);
		const storedFile = stored && hunkByKey.get(hunkOperandIdentityKey(stored))?.file;
		if (storedFile?.item.id !== itemId) return;

		dispatch(
			projectSlice.actions.selectDiff({
				projectId,
				selection: assert(storedFile.hunks[0]).operand,
			}),
		);
		viewerRef.current?.scrollTo({ type: "item", id: itemId, align: "nearest" });
	};

	const handleSetCollapsed = (itemId: string) => (collapsed: boolean) => {
		setManualCollapse(itemId, collapsed);
		if (collapsed && !collapsedItems.has(itemId)) handOffCollapsedSelection(itemId);
	};

	const handleSetReviewed =
		(itemId: string, path: string, version: number) => (reviewed: boolean) => {
			setFilesReviewed({
				projectId,
				contextId: weakFileParentIdentityKey(fileParent),
				files: [{ path, version }],
				reviewed,
			});
			setManualCollapse(itemId, undefined);
			if (reviewed && !collapsedItems.has(itemId)) handOffCollapsedSelection(itemId);
		};

	// We must change the version for updates to the collapsed property to be respected. The versions
	// should be as stable as possible, collapsed or not, for performance. The selected flag is
	// hashed in so the header re-renders when the selection enters or leaves a folded file.
	const enhanceCollapsed = <T,>(
		item: CodeViewDiffItem<T>,
		selected: boolean,
	): CodeViewDiffItem<T> => ({
		...item,
		collapsed: true,
		// We always use versions.
		version: combineHashes(assert(item.version), selected ? 2 : 1),
	});

	// Hoisted from the JSX so the rebuild runs when folds or the folded
	// selection change, not on every render of this component.
	const displayItems =
		collapsedItems.size === 0
			? items
			: items.map((item) =>
					collapsedItems.has(item.id)
						? enhanceCollapsed(item, item.id === selectedFoldedFileId)
						: item,
				);

	return items.length === 0 ? (
		<p className="text-13">No changes.</p>
	) : (
		<>
			<CodeView
				ref={viewerRef}
				renderCustomHeader={(item) => {
					if (item.type === "file") throw new Error("Only diff items may be rendered");

					const file = fileByItemId.get(item.id);
					// CodeView may briefly hold onto stale snapshots of our data.
					if (!file) return <div style={{ height: codeViewItemMetrics.diffHeaderHeight }} />;

					const { version } = file.item;
					if (version === undefined) throw new Error("Diff view item missing version");

					const allReviewedVersions = reviewedFiles.get(file.change.path);
					const hasReviewedThisVersion = !!allReviewedVersions?.has(version);
					const reviewState =
						allReviewedVersions !== undefined
							? hasReviewedThisVersion
								? "reviewed"
								: "changed"
							: null;

					return (
						<DiffFileHeader
							projectId={projectId}
							item={item}
							operand={file.operand}
							change={file.change}
							hasDiff={item.fileDiff.hunks.length !== 0}
							collapsed={item.collapsed ?? false}
							reviewState={reviewState}
							selected={item.id === selectedFoldedFileId}
							setCollapsed={handleSetCollapsed(item.id)}
							setReviewed={handleSetReviewed(item.id, file.change.path, version)}
						/>
					);
				}}
				renderGutterUtility={(getHoveredLine, item) => {
					// We don't currently support annotations on branches.
					if (fileParent._tag === "Branch") return;

					const handleClick = () => {
						const badlyTypedLine = getHoveredLine();
						if (!badlyTypedLine || !("side" in badlyTypedLine)) return;
						const line = badlyTypedLine as GetHoveredLineResult<"diff">;

						const file = fileByItemId.get(item.id);
						if (!file) return;

						const id = crypto.randomUUID();
						newFocusableAnnotationIdRef.current = id;

						createComment({
							projectId,
							comment: {
								id,
								path: file.operand.path,
								commitChangeId: fileParent._tag === "Commit" ? fileParent.changeId : null,
								side: annotationSideToDiffSide(line.side),
								lineNumber: line.lineNumber,
								payload: "",
							},
						});
					};

					return (
						<button
							type="button"
							onClick={handleClick}
							aria-label="Annotate"
							className={classes(
								getButtonClassName({ variant: "pop", size: "small", iconOnly: true }),
								styles.annotate,
							)}
						>
							<Icon name="plus" />
						</button>
					);
				}}
				renderAnnotation={(anno, item) => {
					if (!isDiffAnnotation<Annotation>(anno))
						throw new Error("Only diff items may be rendered");

					const file = fileByItemId.get(item.id);
					if (!file) return null;

					const annotations = annotationsByPath.get(file.operand.path) ?? [];
					const annotationId = anno.metadata.id;
					const annotation = annotations.find(({ id }) => id === annotationId);
					if (!annotation) return null;

					return (
						<AnnotationCard
							projectId={projectId}
							annotation={annotation}
							path={file.operand.path}
							fileParent={fileParent}
							annotationsByPath={annotationsByPath}
							focusAnnotationIdRef={newFocusableAnnotationIdRef}
							selectionScopeRef={selectionScopeRef}
						/>
					);
				}}
				onScroll={selectFileAtViewportTop}
				className={styles.diffContents}
				items={displayItems}
				selectedLines={selectedRange}
				onSelectedLinesChange={handleLinesSelected}
				options={{
					diffStyle: diffStyle ?? defaultSettings.diffStyle,
					disableBackground: !(diffBackgrounds ?? defaultSettings.diffBackground),
					lineDiffType: settings?.lineDiffType ?? defaultSettings.lineDiffType,
					overflow: diffOverflow ?? defaultSettings.diffOverflow,
					themeType: settings?.theme ?? defaultSettings.theme,
					stickyHeaders: true,
					enableLineSelection: true,
					// Manually wire these up instead of using renderGutterUtility to separate annotations from
					// selections.
					onLineEnter: ({ numberElement }) => {
						const slot = document.createElement("slot");
						slot.name = "gutter-utility-slot";
						slot.setAttribute("data-gutter-utility-slot", "");
						numberElement.appendChild(slot);
					},
					onLineLeave: ({ numberElement }) => {
						numberElement.querySelector(':scope > slot[name="gutter-utility-slot"]')?.remove();
					},
					layout: codeViewLayout,
					// This appears to validate before our custom header has been slotted, in which case - if
					// our metrics are correct - we should see deltas in multiples of our custom header height
					// as defined in the metrics. We'll see an additional set of logs if there are other issues
					// with our metrics.
					__devOnlyValidateItemHeights: false,
					onPostRender: handleDiffPostRender,
					itemMetrics: codeViewItemMetrics,
					unsafeCSS: `
          :host {
            background-color: transparent;
            /* Inherited, so this reaches the code inside the shadow root — which is the
               only way in, since ligatures are not one of Pierre's options. */
            font-variant-ligatures: ${
							(settings?.diffLigatures ?? defaultSettings.diffLigatures) ? "normal" : "none"
						};
          }

          [data-diffs-header="custom"] {
            background-color: var(--bg-1);
          }

          [data-diff] {
            border-width: 0 1px 1px 1px;
            border-style: solid;
            border: none;
          }

          [data-column-number] {
            --mix-selection-light: 0%;
            --mix-selection-dark: 0%;

            cursor: default;
          }

          ${diffGutterUnsafeCSS}
        `,
				}}
				style={{
					"--diffs-font-family": settings?.diffFontFamily ?? defaultSettings.diffFontFamily,
					"--diffs-font-size": `${settings?.diffFontSize ?? defaultSettings.diffFontSize}px`,
					"--diffs-tab-size": `${settings?.diffTabSize ?? defaultSettings.diffTabSize}`,
				}}
			/>

			{diffGutterPortals}
		</>
	);
};

type DiffFileHeaderProps = {
	projectId: string;
	item: CodeViewDiffItem<unknown>;
	operand: FileOperand;
	change: TreeChange;
	hasDiff: boolean;
	collapsed: boolean;
	reviewState: "reviewed" | "changed" | null;
	/** Whether the folded file's stand-in hunk holds the diff selection. */
	selected: boolean;
	setCollapsed: (collapsed: boolean) => void;
	setReviewed: (reviewed: boolean) => void;
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

	const collapseLabel = p.collapsed ? "Unfold" : "Fold";
	const reviewLabel =
		p.reviewState === "reviewed"
			? "Reviewed"
			: p.reviewState === "changed"
				? "Needs review"
				: "Not reviewed";

	return (
		<OperationSourceC projectId={p.projectId} source={fileOperand(p.operand)} outline="inside">
			<header
				onContextMenu={(event) => {
					void showNativeContextMenu(event, menuItems);
				}}
				className={classes(
					styles.fileHeader,
					(p.collapsed || !p.hasDiff) && styles.lone,
					p.selected && styles.fileHeaderSelected,
				)}
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
							<Tooltip.Popup
								render={<TooltipPopup kbd={diffHotkeys.toggleFoldFile.hotkey} kbdScope="diff" />}
							>
								{collapseLabel}
							</Tooltip.Popup>
						</Tooltip.Positioner>
					</Tooltip.Portal>
				</Tooltip.Root>
				<h4 className={classes("text-13", styles.filePath)}>
					<FileIcon fileName={fileName} className={styles.icon} />
					{fileName}
					{directoryPath !== null && <span className={styles.pathInit}>{directoryPath}</span>}
				</h4>
				<button
					type="button"
					aria-pressed={p.reviewState === "changed" ? "mixed" : p.reviewState === "reviewed"}
					className={classes(
						getButtonClassName({ variant: "outline", size: "small" }),
						styles.fileReview,
					)}
					onClick={() => p.setReviewed(p.reviewState !== "reviewed")}
				>
					<span className={styles.fileReviewIndicator} aria-hidden="true">
						{p.reviewState !== null && (
							<Icon size={10} name={p.reviewState === "reviewed" ? "tick" : "minus"} />
						)}
					</span>
					{reviewLabel}
				</button>
				<ChangeTypeBadge type={p.item.fileDiff.type} />
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

const FilesToggle: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const dispatch = useAppDispatch();
	const filesVisible = useAppSelector((state) =>
		projectSlice.selectors.selectFilesVisible(state, projectId),
	);

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				render={
					<button
						type="button"
						className={getButtonClassName({ iconOnly: true, variant: "ghost" })}
						aria-label={workspaceHotkeys.toggleFiles.meta.name}
						aria-pressed={filesVisible}
						onClick={() => dispatch(projectSlice.actions.toggleFiles({ projectId }))}
					>
						{filesVisible ? <Icon name="files-sidebar" /> : <Icon name="sidebar-narrow" />}
					</button>
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
						pressed={(diffOverflow ?? defaultSettings.diffOverflow) === "wrap"}
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
						pressed={diffBackgrounds ?? defaultSettings.diffBackground}
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
						value={[diffStyle ?? defaultSettings.diffStyle]}
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

/**
 * Kept whole and out of the component so the compiler can memoise the layout on
 * its inputs; derived in render, the rows — and the navigation index built from
 * them — take a fresh identity every time anything else about the pane changes.
 *
 * The filter narrows the file list only; the diff itself keeps every file, so
 * the list stays a way of reaching a file rather than a way of hiding one.
 */
const buildFilesRows = ({
	filesItems,
	filter,
	mode,
	collapsedDirectories,
}: {
	filesItems: Array<FileRowItem>;
	filter: string | null;
	mode: FileDisplayMode;
	collapsedDirectories: Record<string, true>;
}): Array<FileTreeRow<FileRowItem>> =>
	buildFileTreeRows({
		items: filesItems.filter((item) => pathMatchesFilter(item.path, filter)),
		mode,
		collapsedDirectories,
	});

const Diff: FC<{
	changes: Array<TreeChange>;
	filesVisible: boolean;
	filesItems: Array<FileRowItem>;
	/** The selected commit's unresolved conflicts, if it has any. */
	conflicts?: Array<ConflictedFile>;
	/** Its conflicted files that can only be resolved in edit mode. */
	manualConflicts?: Array<ManualConflict>;
	/** True while `conflicts` still shows the replaced commit's hunks. */
	conflictsStale?: boolean;
	onActiveFileSelection: (itemId: string, firstHunk: HunkOperand | null) => void;
	onPassiveFileSelection: (selection: string) => void;
	selection: Operand;
	projectId: string;
	viewerRef: RefObject<DiffViewerHandle | null>;
	didScrollToViaFileRef: RefObject<boolean>;
	headerSlot?: ReactNode;
}> = ({
	changes: unsortedChanges,
	filesVisible,
	filesItems,
	conflicts = EMPTY_CONFLICTS,
	manualConflicts = EMPTY_MANUAL,
	conflictsStale = false,
	onPassiveFileSelection,
	selection,
	projectId,
	onActiveFileSelection,
	viewerRef,
	didScrollToViaFileRef,
	headerSlot,
}) => {
	const selectionScopeRef = useRef<HTMLDivElement>(null);
	const dispatch = useAppDispatch();
	const { mutate: setFilesReviewed } = useSetFilesReviewed();
	const [manualCollapseByItem, setManualCollapseByItem] = useState<Map<string, boolean>>(new Map());
	const setManualCollapse = (itemId: string, collapsed: boolean | undefined): void => {
		setManualCollapseByItem((current) => {
			if (collapsed === current.get(itemId)) return current;

			const next = new Map(current);
			if (collapsed === undefined) next.delete(itemId);
			else next.set(itemId, collapsed);
			return next;
		});
	};
	// One mutation for the batch bar and every card, so `isPending` means "a
	// resolution is in flight" rather than "this one's is". Each apply rewrites
	// the commit, and a second started meanwhile would address the id it replaced.
	const { mutate: resolveConflict, isPending: resolvingConflict } = useResolveCommitConflictHunks();
	const changes = useMemo(
		() => unsortedChanges.toSorted((a, b) => compareFilePaths(a.path, b.path)),
		[unsortedChanges],
	);

	const { data: renderAllFiles } = useSuspenseQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.unidiff ?? defaultSettings.unidiff,
	});

	const canShowFiles = useAppSelector((state) =>
		projectSlice.selectors.selectCanShowFiles(state, projectId),
	);
	const detailsFullWindow = useAppSelector(interfaceSlice.selectors.selectDetailsFullWindow);

	// Change stats live in the files panel, or — in the uncommitted scope, which has no files
	// panel — in the outline's "Uncommitted" row. Surface them in the toolbar below whenever
	// whichever of those owns them is hidden, so they never disappear entirely.
	const statsShownElsewhere = canShowFiles ? filesVisible : !detailsFullWindow;

	const filesFilter = useAppSelector((state) =>
		projectSlice.selectors.selectFilesFilter(state, projectId),
	);
	const fileDisplayMode = useFileDisplayMode();
	const filesCollapsedDirectories = useAppSelector((state) =>
		projectSlice.selectors.selectFilesCollapsedDirectories(state, projectId),
	);
	// As with `fileParent` below, the compiler leaves this derivation outside its
	// memo blocks here, and the rows carry the identity the file list and its
	// navigation index are keyed on — so it is memoised by hand.
	const filesRows = useMemo(
		() =>
			buildFilesRows({
				filesItems,
				filter: filesFilter,
				mode: fileDisplayMode,
				collapsedDirectories: filesCollapsedDirectories,
			}),
		[filesItems, filesFilter, fileDisplayMode, filesCollapsedDirectories],
	);
	const filesNavigationIndex = useMemo(() => fileTreeNavigationIndex(filesRows), [filesRows]);
	const filesSelection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionFiles(state, projectId, filesNavigationIndex),
	);

	// At time of writing React Compiler cannot statically analyse that these are pure derivations of
	// the outline selection, even with the helpers inlined, hence manual memoisation.
	const fileParent = useMemo(
		() =>
			Match.value(selection).pipe(
				Match.tags({
					Branch: ({ branchRef }) => branchFileParent({ branchRef }),
					File: ({ parent }) => parent,
					Commit: ({ commitId, changeId }) => commitFileParent({ commitId, changeId }),
				}),
				Match.orElseAbsurd,
			),
		[selection],
	);
	const reviewedFilesContextId = weakFileParentIdentityKey(fileParent);
	const { data: reviewedFiles } = useSuspenseQuery(
		reviewedFilesQueryOptions(projectId, reviewedFilesContextId),
	);

	// Eagerly fetch all diffs regardless of unidiff setting, both for UX and for the total line
	// stats.
	const { treeChangeDiffs, lineStats } = useSuspenseQueries({
		queries: changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
		combine: (results) => {
			const treeChangeDiffs = results.map((result) => result.data);
			return { treeChangeDiffs, lineStats: getLineStats(treeChangeDiffs) };
		},
	});

	const { data: annotationsByPath = EMPTY_ANNOTATIONS_BY_PATH } = useQuery({
		...commentsQueryOptions(projectId),
		select: (comments) => annotationsByPathForScope(comments, fileParent),
	});

	// A directory row stands for the first file below it, so the diff has
	// something to show while the cursor rests on a folder.
	const activeFilePath =
		selection._tag === "File" ? selection.path : selectedFilePath(filesRows, filesSelection);

	// Keyed on the file's index, not its path: scrolling moves the selection, so
	// keying on path reparsed every file per boundary crossed. `null` is
	// render-all, distinct from the -1 of a path matching no file.
	const shownFileIndex = renderAllFiles
		? null
		: changes.findIndex((change) => change.path === activeFilePath);
	const preparedDiffFiles = useMemo(
		() => prepareDiffFiles({ fileParent, changes, treeChangeDiffs }),
		[fileParent, changes, treeChangeDiffs],
	);

	const diffViewSansAnno = useMemo(
		() =>
			getDiffView(
				shownFileIndex === null
					? preparedDiffFiles
					: preparedDiffFiles.slice(shownFileIndex, shownFileIndex + 1),
			),
		[shownFileIndex, preparedDiffFiles],
	);

	const diffView = withAnnotations(diffViewSansAnno, annotationsByPath);
	const activeFileItemId =
		activeFilePath === null
			? null
			: (diffViewSansAnno.fileByPath.get(activeFilePath)?.item.id ?? null);

	const allFilesReviewed =
		preparedDiffFiles.length > 0 &&
		preparedDiffFiles.every(({ change, version }) => reviewedFiles.get(change.path)?.has(version));

	const toggleAllFilesReviewed = (): void => {
		setManualCollapseByItem(new Map());
		setFilesReviewed({
			projectId,
			contextId: reviewedFilesContextId,
			files: preparedDiffFiles.map(({ change, version }) => ({ path: change.path, version })),
			reviewed: !allFilesReviewed,
		});
	};

	// The diff panel resolves this selection for the viewer; the ruler wants it in
	// file line numbers, which is what the hunk's own range already holds.
	const diffSelection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionDiff(state, projectId, diffViewSansAnno.navigationIndex),
	);
	const minimapSelection = useMemo((): MinimapSelection | null => {
		if (!diffSelection) return null;

		const key = hunkOperandIdentityKey(diffSelection);
		const selected = diffViewSansAnno.hunkByKey.get(key)?.selectedLines;
		if (!selected) return null;

		return {
			itemId: selected.id,
			side: selected.range.side ?? "additions",
			start: selected.range.start,
			end: selected.range.end,
		};
	}, [diffSelection, diffViewSansAnno]);

	const activateRow = (selection: string) => {
		onPassiveFileSelection(selection);

		const path = selectedFilePath(filesRows, selection);
		const file = path === null ? undefined : diffViewSansAnno.fileByPath.get(path);
		if (!file) return;

		onActiveFileSelection(file.item.id, file.hunks[0]?.operand ?? null);
	};

	const filesPanelRef = useRef<HTMLDivElement>(null);
	const filesTreeRef = useRef<HTMLDivElement>(null);
	const fileFilter = useFileFilter({
		filter: filesFilter,
		setFilter: (filter) => dispatch(projectSlice.actions.setFilesFilter({ projectId, filter })),
		inputId: "files-filter-input",
		scope: "files",
		selection: filesSelection,
		firstPath: filesRows[0]?.path,
		onEnterList: activateRow,
		panelRef: filesPanelRef,
		listRef: filesTreeRef,
		enabled: filesVisible && changes.length > 0,
	});

	const { data: diffSettings } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => ({
			diffBackground: cfg.diffBackground,
			diffOverflow: cfg.diffOverflow,
			diffStyle: cfg.diffStyle,
			diffTabSize: cfg.diffTabSize,
			minimap: cfg.minimap,
		}),
	});

	const { mutate: saveGUISettings } = useSaveGUISettings();

	const diffContentsEl = useRef<HTMLElement | null>(null);
	const [canUseSplitDiff, setCanUseSplitDiff] = useState<boolean | undefined>();
	const [wrapColumns, setWrapColumns] = useState<number | null>(null);

	// Wrapping stretches a long line over several rows, which the minimap has to
	// model or its marks drift down the file it is mapping.
	const wraps = (diffSettings?.diffOverflow ?? defaultSettings.diffOverflow) === "wrap";

	// Split and unified lay hunks out differently, so the minimap has to model
	// whichever style the viewer is actually rendering.
	const diffStyle = canUseSplitDiff
		? (diffSettings?.diffStyle ?? defaultSettings.diffStyle)
		: "unified";

	const tabSize = diffSettings?.diffTabSize ?? defaultSettings.diffTabSize;

	// The minimap maps whatever the viewer holds — every file, or the one file
	// shown at a time. Keyed on that file's index rather than its path so the
	// map doesn't rebuild as scrolling moves the selection through the list.
	const shownIndex = renderAllFiles
		? -1
		: changes.findIndex((change) => change.path === activeFilePath);

	const minimapShown = diffSettings?.minimap ?? defaultSettings.minimap;
	// Modelling the map reads every line of the diff, so a ruler nobody asked for
	// shouldn't be parsed for either.
	const minimapFiles = useMemo(
		() =>
			minimapShown
				? getMinimapFiles({
						files:
							shownIndex < 0
								? preparedDiffFiles
								: preparedDiffFiles.slice(shownIndex, shownIndex + 1),
						diffStyle,
						tabSize,
						wrapColumns,
					})
				: [],
		[minimapShown, shownIndex, preparedDiffFiles, diffStyle, tabSize, wrapColumns],
	);

	useHotkeys([
		{
			hotkey: diffHotkeys.toggleDiffStyle.hotkey,
			callback: () =>
				saveGUISettings({
					diffStyle:
						(diffSettings?.diffStyle ?? defaultSettings.diffStyle) === "split"
							? "unified"
							: "split",
				}),
			options: {
				conflictBehavior: "allow",
				enabled: canUseSplitDiff,
				meta: diffHotkeys.toggleDiffStyle.meta,
			},
		},
	]);

	// Both of these are facts about the rendered pane rather than about the diff,
	// so they are measured on the same resize rather than derived.
	useLayoutEffect(() => {
		const el = diffContentsEl.current;
		if (!el) return;

		const measure = () => {
			setCanUseSplitDiff(el.getBoundingClientRect().width >= 700);

			if (!wraps) {
				setWrapColumns(null);
				return;
			}

			// Held only once it can be read: a resize that lands between renders would
			// otherwise drop the count and unwrap the whole model for a frame.
			const viewer = viewerRef.current?.getInstance();
			const columns = viewer ? measureWrapColumns(viewer) : null;
			if (columns !== null) setWrapColumns(columns);
		};

		measure();

		const resizeObserver = new ResizeObserver(measure);
		resizeObserver.observe(el);

		return () => resizeObserver.disconnect();
	}, [diffContentsEl, viewerRef, wraps, diffViewSansAnno]);

	const layoutId = `project=${projectId}:details`;
	const panelIds: Array<PanelId> = filesVisible ? ["files-panel", "diff-panel"] : ["diff-panel"];
	const diffLayout = useDefaultLayout({
		id: layoutId,
		panelIds,
	});

	return (
		<div className={styles.diffTab}>
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
							defaultSize={320}
							minSize={220}
							groupResizeBehavior="preserve-pixel-size"
						>
							<div className={styles.filesPanelContent} ref={filesPanelRef}>
								{fileFilter.rowProps === null ? (
									<ChangesHeaderRow
										projectId={projectId}
										fileParent={fileParent}
										changes={changes}
										lineStats={lineStats}
										onOpenFilter={fileFilter.open}
									/>
								) : (
									<FileFilterRow {...fileFilter.rowProps} />
								)}
								<Scroller
									withSeparator
									className={styles.filesScrollerArea}
									viewportClassName={styles.diffFiles}
								>
									<FilesTree
										selectionScope="files"
										onRowSelection={activateRow}
										projectId={projectId}
										rows={filesRows}
										collapsedDirectories={filesCollapsedDirectories}
										onToggleDirectoryCollapsed={(path) =>
											dispatch(
												projectSlice.actions.toggleFilesDirectoryCollapsed({ projectId, path }),
											)
										}
										selection={filesSelection}
										navigationIndex={filesNavigationIndex}
										fileParent={fileParent}
										emptyLabel={
											filesFilter !== null && filesItems.length > 0
												? "No matching files."
												: undefined
										}
										ref={filesTreeRef}
									/>
								</Scroller>
							</div>
						</Panel>
						<ResizeHandle />
					</>
				)}

				<Panel id={"diff-panel" satisfies PanelId} minSize={300} className={styles.panel}>
					<div className={classes(styles.actions, !filesVisible && styles.filesHidden)}>
						{canShowFiles && <FilesToggle />}

						{headerSlot}

						{!statsShownElsewhere && (
							<ChangeStats fileCount={changes.length} lineStats={lineStats} />
						)}

						<Toolbar.Root aria-label="Diff controls" className={styles.diffControls}>
							<Toolbar.Button
								className={getButtonClassName({ variant: "outline" })}
								disabled={preparedDiffFiles.length === 0}
								onClick={toggleAllFilesReviewed}
							>
								{allFilesReviewed ? "Mark all unreviewed" : "Mark all reviewed"}
							</Toolbar.Button>
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

					{/* One panel child, so `.panel`'s two-row grid still sizes the
					    diff: the bar is an auto row inside this, not a third row
					    that would take the diff's. */}
					<div className={styles.diffArea}>
						{fileParent._tag === "Commit" && (
							<ConflictBar
								projectId={projectId}
								commitId={fileParent.commitId}
								conflicts={conflicts}
								manual={manualConflicts}
								busy={resolvingConflict || conflictsStale}
								onResolve={(specs) =>
									resolveConflict({ projectId, commitId: fileParent.commitId, specs })
								}
							/>
						)}

						<div
							data-selection-scope={"diff" satisfies SelectionScope}
							// oxlint-disable-next-line jsx_a11y/no-noninteractive-tabindex -- Revisit this when we add hunk/line selection.
							tabIndex={0}
							className={styles.diffContentsContainer}
							ref={useMergedRefs(selectionScopeRef, diffContentsEl, useAutofocusSelectionScope())}
						>
							<DiffContents
								activeFileItemId={activeFileItemId}
								onViewerFileSelection={onPassiveFileSelection}
								fileParent={fileParent}
								projectId={projectId}
								diffView={diffView}
								annotationsByPath={annotationsByPath}
								diffBackgrounds={diffSettings?.diffBackground}
								diffOverflow={diffSettings?.diffOverflow}
								diffStyle={diffStyle}
								reviewedFiles={reviewedFiles}
								manualCollapseByItem={manualCollapseByItem}
								setManualCollapse={setManualCollapse}
								setFilesReviewed={setFilesReviewed}
								selectionScopeRef={selectionScopeRef}
								viewerRef={viewerRef}
								didScrollToViaFileRef={didScrollToViaFileRef}
							/>

							{minimapShown && (
								<DiffMinimap
									viewerRef={viewerRef}
									files={minimapFiles}
									diffStyle={diffStyle}
									annotationsByPath={annotationsByPath}
									selection={minimapSelection}
								/>
							)}
						</div>
					</div>
				</Panel>
			</Group>
		</div>
	);
};

const CopyableId: FC<{
	label: string;
	icon: IconName;
	displayValue: string;
	copyValue: string;
}> = ({ label, icon, displayValue, copyValue }) => {
	const [copied, setCopied] = useState(false);
	const resetTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

	const handleCopy = () => {
		void window.lite.clipboardWriteText(copyValue);
		setCopied(true);

		if (resetTimeoutRef.current !== null) clearTimeout(resetTimeoutRef.current);
		resetTimeoutRef.current = setTimeout(() => setCopied(false), 1500);
	};

	useLayoutEffect(
		() => () => {
			if (resetTimeoutRef.current !== null) clearTimeout(resetTimeoutRef.current);
		},
		[],
	);

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				className={styles.commitDetailsMetaSha}
				onClick={handleCopy}
				render={<button type="button" aria-label={label} />}
			>
				<Icon size={14} name={copied ? "tick" : icon} />
				<span>{copied ? "Copied!" : displayValue}</span>
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>{label}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const CommitDetailsSkeleton: FC = () => {
	const detailsFullWindow = useAppSelector(interfaceSlice.selectors.selectDetailsFullWindow);

	return (
		<div className={styles.container}>
			<div className={styles.headerWrap}>
				<div className={styles.titleRow}>
					{detailsFullWindow && <TopLeftControls />}

					<div className={styles.title}>
						<SelectionScopeKbd hotkey="0" scope="details" />
						<Icon name="commit" />
						<h3 className={classes("text-15", "text-semibold")}>Loading…</h3>
					</div>
				</div>
			</div>
		</div>
	);
};

const CommitDetails: FC<{
	selection: Extract<Operand, { _tag: "Commit" }>;
	onActiveFileSelection: (itemId: string, firstHunk: HunkOperand | null) => void;
	viewerRef: RefObject<DiffViewerHandle | null>;
	didScrollToViaFileRef: RefObject<boolean>;
}> = ({ selection, onActiveFileSelection, viewerRef, didScrollToViaFileRef }) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
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
	const commitBodyId = useId();

	const { data: commitDetails } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId: selection.commitId }),
	);

	const { data: conflictsData, isPlaceholderData: conflictsStale } = useQuery({
		...commitConflictsQueryOptions({
			projectId,
			commitId: selection.commitId,
			enabled: commitDetails.commit.hasConflicts,
		}),
		// A resolution rewrites the commit and re-keys this query; holding the
		// previous result keeps the bar — and the open resolution dialog —
		// mounted across the refetch instead of flashing them away. Rewrites
		// are the only bridge: selecting another commit remounts this component.
		// While the placeholder shows, the hunks belong to the replaced commit,
		// so resolution actions are held busy rather than let address dead ids.
		placeholderData: keepPreviousData,
	});
	// The placeholder outlives the last conflict, so a commit that just
	// normalized must not keep reporting the resolved ones.
	const conflicts = commitDetails.commit.hasConflicts ? conflictsData : undefined;

	// The commit's changes as they are: the auto-resolution falls back to the
	// parent at every conflict, so a conflicted region contributes nothing here
	// until it is resolved toward the intended side — the bar flags it and
	// hosts the resolution dialog.
	const changes = commitDetails.changes;

	const filesItems = useMemo(
		() => getCommitFileRowItems({ commitDetails, manual: conflicts?.manual }),
		[commitDetails, conflicts],
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

	const selectFile = (selection: string) => {
		dispatch(projectSlice.actions.selectFiles({ projectId, selection }));
	};

	return (
		<div className={styles.container}>
			<div className={styles.headerWrap}>
				<div className={styles.titleRow}>
					{detailsFullWindow && <TopLeftControls />}

					<div className={styles.title}>
						<SelectionScopeKbd hotkey="0" scope="details" />
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
										aria-controls={commitBodyId}
										aria-expanded={!commitBodyCollapsed}
										aria-label={commitBodyCollapsed ? "Expand commit body" : "Collapse commit body"}
										aria-pressed={!commitBodyCollapsed}
										className={classes(
											getButtonClassName({
												variant: commitBodyCollapsed ? "outline" : "gray",
												iconOnly: true,
												size: "small",
											}),
											styles.commitBodyToggle,
										)}
										onClick={() => setCommitBodyCollapsed(!commitBodyCollapsed)}
									>
										<Icon name="kebab" />
									</Tooltip.Trigger>
									<Tooltip.Portal>
										<Tooltip.Positioner sideOffset={4}>
											<Tooltip.Popup render={<TooltipPopup />}>
												{commitBodyCollapsed ? "Expand commit body" : "Collapse commit body"}
											</Tooltip.Popup>
										</Tooltip.Positioner>
									</Tooltip.Portal>
								</Tooltip.Root>
							)}
						</h3>
					</div>
				</div>

				{body !== undefined && !commitBodyCollapsed && (
					<p
						id={commitBodyId}
						className={classes("text-monospace", "text-body", styles.commitMessageBody)}
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
						<span title={commitDetails.commit.author.email}>
							{commitDetails.commit.author.name}
						</span>{" "}
						at {fmtDate}
					</span>
					<CopyableId
						label="Copy change ID"
						icon="finger-print"
						displayValue={shortCommitId(commitDetails.commit.changeId)}
						copyValue={commitDetails.commit.changeId}
					/>
					<CopyableId
						label="Copy commit ID"
						icon="hash"
						displayValue={shortCommitId(commitDetails.commit.id)}
						copyValue={commitDetails.commit.id}
					/>
				</div>
			</div>

			<Diff
				changes={changes}
				filesVisible={filesVisible}
				filesItems={filesItems}
				conflicts={conflicts?.files}
				manualConflicts={conflicts?.manual}
				conflictsStale={conflictsStale}
				onPassiveFileSelection={selectFile}
				selection={selection}
				projectId={projectId}
				onActiveFileSelection={onActiveFileSelection}
				viewerRef={viewerRef}
				didScrollToViaFileRef={didScrollToViaFileRef}
			/>
		</div>
	);
};

const BranchDetails: FC<{
	selection: Extract<Operand, { _tag: "Branch" }>;
	onActiveFileSelection: (itemId: string, firstHunk: HunkOperand | null) => void;
	viewerRef: RefObject<DiffViewerHandle | null>;
	didScrollToViaFileRef: RefObject<boolean>;
}> = ({ selection, onActiveFileSelection, viewerRef, didScrollToViaFileRef }) => {
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
	const branchRef = decodeBytes(selection.branchRef);
	const branchName = branchDetailsParams(branchRef).branchName;
	const branchTab = useAppSelector((state) =>
		projectSlice.selectors.selectBranchTab(state, projectId, branchName),
	);

	const setBranchTab = (tab: BranchTab) => {
		dispatch(projectSlice.actions.setSelectedBranchTab({ projectId, branchName, tab }));
	};

	// Per-PR by construction: BranchDetails is keyed on the branch identity,
	// so a selection change remounts this component and resets the mode.
	const [prEditing, setPrEditing] = useState(false);

	const togglePrEdit = () => {
		if (!prEditing) setBranchTab("pr");
		setPrEditing(!prEditing);
	};

	const ref = useRef<HTMLDivElement>(null);

	useHotkeys([
		{
			hotkey: "[",
			callback: () => {
				switch (branchTab) {
					case "diff": {
						setBranchTab("pr");
						break;
					}
					case "pr": {
						setBranchTab("diff");
						break;
					}
					default:
						branchTab satisfies never;
				}
			},
			options: {
				conflictBehavior: "allow",
				target: ref,
			},
		},
		{
			hotkey: "]",
			callback: () => {
				switch (branchTab) {
					case "diff": {
						setBranchTab("pr");
						break;
					}
					case "pr": {
						setBranchTab("diff");
						break;
					}
					default:
						branchTab satisfies never;
				}
			},
			options: {
				conflictBehavior: "allow",
				target: ref,
			},
		},
	]);

	const selectFile = (selection: string) => {
		dispatch(projectSlice.actions.selectFiles({ projectId, selection }));
	};

	// Use push status of segment, not branch details; something about remote
	// tracking refs.
	const branchCtx = headInfoIndex?.branchContextByRefBytes(selection.branchRef);
	const parentSegment = branchCtx?.stack.segments[branchCtx.segmentIndex + 1];
	const targetBranch =
		!parentSegment || parentSegment.pushStatus === "integrated"
			? headInfo?.target?.remoteTrackingRef.displayName
			: parentSegment.pushStatus === "completelyUnpushed"
				? undefined
				: parentSegment.refName?.displayName;

	return (
		<div className={styles.container} ref={ref}>
			<div className={styles.headerWrap}>
				<div className={styles.titleRow}>
					{detailsFullWindow && <TopLeftControls />}

					<div className={styles.title}>
						<SelectionScopeKbd hotkey="0" scope="details" />
						<Icon name="branch" />
						<h3 className={classes("text-15", "text-semibold")}>{branchName}</h3>
					</div>
				</div>

				<div className={classes(styles.tabsRow, branchTab === "pr" && styles.tabsRowPrCap)}>
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

					{branchTab === "pr" && !!forgeInfo?.capabilities.prService && (
						<Suspense>
							<SuspenseQuery
								{...listReviewsQueryOptions({
									projectId,
									cacheConfig: "noCache",
								})}
							>
								{({ data }) => {
									const review = data.reviewsBySourceBranch.get(branchName);
									if (!review) return null;

									return (
										<div className={styles.tabsRowRight}>
											<PullRequestPrimaryAction
												projectId={projectId}
												reviewId={review.number}
												isDraft={review.draft}
												autoMergeEnabled={review.autoMergeEnabled}
												isEditing={prEditing}
												onToggleEdit={togglePrEdit}
											/>
										</div>
									);
								}}
							</SuspenseQuery>
						</Suspense>
					)}
				</div>
			</div>

			<Suspense fallback={<div className={classes(styles.loadingTab, "text-13")}>Loading…</div>}>
				{branchTab === "pr" ? (
					<div className={styles.prTab}>
						{!forgeInfo?.capabilities.prService ||
						targetBranch === undefined ||
						branchCtx?.segment.pushStatus === "completelyUnpushed" ? (
							<PullRequestForm
								key={branchName}
								body={null}
								projectId={projectId}
								reviewId={null}
								sourceBranch={branchName}
								title={null}
								canSubmit={false}
							/>
						) : (
							<SuspenseQuery
								{...listReviewsQueryOptions({
									projectId,
									cacheConfig: "noCache",
								})}
							>
								{({ data }) => {
									const review = data.reviewsBySourceBranch.get(branchName);

									return !review ? (
										<PullRequestForm
											key={branchName}
											body={null}
											projectId={projectId}
											reviewId={null}
											sourceBranch={branchName}
											title={null}
											canSubmit
										/>
									) : (
										<div className={styles.prLayout}>
											<div className={styles.prMain}>
												<PullRequestDescription
													key={review.number}
													body={review.body}
													projectId={projectId}
													reviewId={review.number}
													sourceBranch={branchName}
													title={review.title}
													canSubmit
													editing={prEditing}
													onDoneEditing={() => setPrEditing(false)}
												/>

												{forgeInfo.capabilities.reviewComments !== false && (
													<PullRequestComments projectId={projectId} review={review} />
												)}
											</div>

											<PullRequestPanel projectId={projectId} review={review} />
										</div>
									);
								}}
							</SuspenseQuery>
						)}
					</div>
				) : (
					<SuspenseQuery {...branchDiffQueryOptions({ projectId, branch: branchRef })}>
						{({ data: branchDiff }) => (
							<Diff
								changes={branchDiff.changes}
								filesVisible={filesVisible}
								filesItems={branchDiff.changes.map((change) =>
									changeFileRowItem({
										change,
										path: change.path,
										dependencyCommitIds: [],
									}),
								)}
								onPassiveFileSelection={selectFile}
								selection={selection}
								projectId={projectId}
								onActiveFileSelection={onActiveFileSelection}
								viewerRef={viewerRef}
								didScrollToViaFileRef={didScrollToViaFileRef}
							/>
						)}
					</SuspenseQuery>
				)}
			</Suspense>
		</div>
	);
};

const FileDetailsSkeleton: FC = () => {
	const detailsFullWindow = useAppSelector(interfaceSlice.selectors.selectDetailsFullWindow);

	return (
		<div className={styles.container}>
			<div className={styles.headerWrap}>
				<div className={styles.titleRow}>
					{detailsFullWindow && <TopLeftControls />}

					<div className={styles.title}>
						<SelectionScopeKbd hotkey="0" scope="details" />
						<Icon name="file" />
						<h3 className={classes("text-15", "text-semibold")}>Uncommitted</h3>
					</div>
				</div>
			</div>

			<div className={classes(styles.loadingTab, "text-13")}>Loading…</div>
		</div>
	);
};

const FileDetails: FC<{
	selection: Extract<Operand, { _tag: "File" }> & {
		parent: Extract<FileParent, { _tag: "UncommittedChanges" }>;
	};
	onActiveFileSelection: (itemId: string, firstHunk: HunkOperand | null) => void;
	viewerRef: RefObject<DiffViewerHandle | null>;
	didScrollToViaFileRef: RefObject<boolean>;
}> = ({ selection, onActiveFileSelection, viewerRef, didScrollToViaFileRef }) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const dispatch = useAppDispatch();
	const detailsFullWindow = useAppSelector(interfaceSlice.selectors.selectDetailsFullWindow);
	const filesVisibleState = useAppSelector((state) =>
		projectSlice.selectors.selectFilesVisible(state, projectId),
	);
	const canShowFiles = useAppSelector((state) =>
		projectSlice.selectors.selectCanShowFiles(state, projectId),
	);
	const filesVisible = canShowFiles && filesVisibleState;
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));
	const filesItems = getChangesFileRowItems(worktreeChanges);
	const changes = filesItems.flatMap((item) => (item._tag === "Change" ? [item.change] : []));

	const selectFile = (selection: string) => {
		dispatch(projectSlice.actions.selectUncommittedFiles({ projectId, selection }));
	};

	const title = (
		<>
			{detailsFullWindow && <TopLeftControls />}

			<div className={styles.title}>
				<SelectionScopeKbd hotkey="0" scope="details" />
				<Icon name="file-diff" />
				<h3 className={classes("text-15", "text-semibold")}>Uncommitted</h3>
			</div>
		</>
	);

	return (
		<div className={classes(styles.container, changes.length > 0 && styles.containerLone)}>
			{changes.length > 0 ? (
				<Diff
					changes={changes}
					filesVisible={filesVisible}
					filesItems={filesItems}
					onPassiveFileSelection={selectFile}
					selection={selection}
					projectId={projectId}
					onActiveFileSelection={onActiveFileSelection}
					viewerRef={viewerRef}
					didScrollToViaFileRef={didScrollToViaFileRef}
					headerSlot={title}
				/>
			) : (
				<div className={styles.headerWrap}>
					<div className={styles.titleRow}>{title}</div>
				</div>
			)}
		</div>
	);
};

export const Details: FC<{
	selection: Operand | null;
	onActiveFileSelection: (itemId: string, firstHunk: HunkOperand | null) => void;
	viewerRef: RefObject<DiffViewerHandle | null>;
	didScrollToViaFileRef: RefObject<boolean>;
}> = ({ selection, onActiveFileSelection, viewerRef, didScrollToViaFileRef }) => {
	if (!selection) return;

	return Match.value(selection).pipe(
		Match.tags({
			Commit: (commit) => (
				<Suspense fallback={<CommitDetailsSkeleton />}>
					<CommitDetails
						key={weakCommitIdentityKey(commit)}
						selection={commit}
						onActiveFileSelection={onActiveFileSelection}
						viewerRef={viewerRef}
						didScrollToViaFileRef={didScrollToViaFileRef}
					/>
				</Suspense>
			),
			Branch: (branch) => (
				<BranchDetails
					key={branchIdentityKey(branch)}
					selection={branch}
					onActiveFileSelection={onActiveFileSelection}
					viewerRef={viewerRef}
					didScrollToViaFileRef={didScrollToViaFileRef}
				/>
			),
		}),
		Match.when({ _tag: "File", parent: { _tag: "UncommittedChanges" } }, (file) => (
			<Suspense fallback={<FileDetailsSkeleton />}>
				<FileDetails
					selection={file}
					onActiveFileSelection={onActiveFileSelection}
					viewerRef={viewerRef}
					didScrollToViaFileRef={didScrollToViaFileRef}
				/>
			</Suspense>
		)),
		Match.orElseAbsurd,
	);
};
