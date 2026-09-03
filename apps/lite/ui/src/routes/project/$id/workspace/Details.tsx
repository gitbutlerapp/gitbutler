import { ResizeHandle } from "#ui/components/ResizeHandle.tsx";
import { startAbsorb, setCursor, useCanShowFiles, useSelection } from "#ui/use-cursor.ts";
import uiStyles from "#ui/components/ui.module.css";
import { SuspenseQuery } from "@suspensive/react-query";
import {
	useAddReviewLabels,
	useCommitUncommitChanges,
	useOpenInProgram,
	useRequestReview,
	useResolveCommitConflictHunks,
	useSaveGUISettings,
} from "#ui/api/mutations.ts";
import {
	type DraftPRExtras,
	draftPRQueryOptions,
	useLandedReviewId,
	usePersistDraftPR,
} from "#ui/pr.ts";
import {
	blobFileQueryOptions,
	branchDiffQueryOptions,
	branchListQueryOptions,
	changesInWorktreeQueryOptions,
	commentsQueryOptions,
	commitConflictsQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	forgeInfoOptions,
	getReviewQueryOptions,
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	listEditorsQueryOptions,
	listReviewsQueryOptions,
	listReviewThreadsQueryOptions,
	treeChangeDiffsQueryOptions,
	workspaceFileQueryOptions,
} from "#ui/api/queries.ts";
import {
	SeenOnArrivalContext,
	useMarkReviewSeenOnView,
	usePrNotificationsLevel,
	useReviewUnread,
	useSeenOnArrival,
} from "#ui/review-seen.ts";
import rowStyles from "./Row.module.css";
import { decodeBytes } from "#ui/api/bytes.ts";
import type { ForgeReview, TargetCommitReview } from "@gitbutler/but-sdk";
import { branchDetailsParams } from "#ui/branch.ts";
import { commitBody, commitTitle, shortCommitId } from "#ui/commit.ts";
import {
	branchFileParent,
	branchIdentityKey,
	type BranchAddress,
	branchAddress,
	commitFileParent,
	type FileAddress,
	fileAddress,
	hunkAddress,
	addressEquals,
	type FileParent,
	type HunkAddress,
	type Address,
	uncommittedChangesFileParent,
	weakCommitIdentityKey,
	weakFileIdentityKey,
	weakFileParentIdentityKey,
} from "#ui/addresses.ts";
import type { DiffLineSelection } from "#ui/cursors.ts";
import { checkedRange, addressSpaceRange } from "#ui/checking.ts";
import type { BranchTab, CheckableAddress } from "#ui/projects/project.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { interfaceSlice } from "#ui/interface/state.ts";
import { Badge } from "#ui/components/Badge.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import {
	PullRequestComments,
	ReviewTimeline,
} from "#ui/routes/project/$id/workspace/PullRequestComments.tsx";
import {
	NewPullRequestPanel,
	PullRequestPanel,
} from "#ui/routes/project/$id/workspace/PullRequestPanel.tsx";
import {
	PullRequestDescription,
	PullRequestForm,
	PullRequestPrimaryAction,
} from "#ui/routes/project/$id/workspace/PullRequestForm.tsx";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { EmptyState } from "#ui/components/EmptyState.tsx";
import { Toggle, ToggleGroup, Toolbar, Tooltip } from "@base-ui/react";
import type {
	CommitDetails as CommitDetailsData,
	ConflictedFile,
	ManualConflict,
	TreeChange,
} from "@gitbutler/but-sdk";
import {
	type CodeViewItem,
	type CodeView as CodeViewClass,
	type CodeViewLineSelection,
	type CodeViewOptions,
	type DiffLineAnnotation,
	type FileContents,
	isDiffAnnotation,
} from "@pierre/diffs";
import { CodeView, type CodeViewHandle, useStableCallback } from "@pierre/diffs/react";
import {
	keepPreviousData,
	useQuery,
	useQueryClient,
	useSuspenseQueries,
	useSuspenseQuery,
} from "@tanstack/react-query";
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
	focusScope,
	type FocusScope,
	useAutofocusScope,
	useAddressSpaceHotkeys,
} from "#ui/focus-scopes.ts";
import { buildIndexByKey, getAdjacent } from "#ui/workspace/address-space.ts";
import { ChangeStats } from "#ui/routes/project/$id/workspace/ChangeStats.tsx";
import { ChangeScale } from "#ui/components/ChangeScale.tsx";
import { DiffStats } from "#ui/components/DiffStats.tsx";
import { ChangesHeaderRow } from "#ui/routes/project/$id/workspace/ChangesHeaderRow.tsx";
import {
	describeLineStats,
	getLineStats,
	patchLineStats,
	type LineStats,
} from "#ui/routes/project/$id/workspace/lineStats.ts";
import { FilesTree } from "#ui/routes/project/$id/workspace/FilesTree.tsx";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
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
	fileTreeAddressSpace,
	selectedFilePath,
	type FileDisplayMode,
	type FileTreeRow,
} from "./file-tree.ts";
import { useFileDisplayMode } from "./useFileDisplayMode.ts";
import { ListFilterRow } from "./ListFilterRow.tsx";
import { useListFilter } from "./useListFilter.ts";
import {
	contiguousSelectionByLine,
	hunkSelectionForLineNavigation,
	lineSelectionsForRange,
	moveSelectedLineRange,
	rangeFromLineGroups,
	selectedLineRangeContainsPoint,
	singleLineSelectionByLine,
	type HunkLineSelection,
	wholeHunkSelectionByLine,
} from "#ui/hunk.ts";
import { showNativeContextMenu, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import { useFileMenuItems } from "#ui/routes/project/$id/workspace/useFileMenuItems.ts";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { getHeadInfoIndex, recordedPullRequest } from "#ui/api/ref-info.ts";
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
import { diffLineTargetFromElement, type DiffLineTarget } from "./diff-line-target.ts";
import { useHunkMenuItems } from "./useHunkMenuItems.ts";
import { useRevealInFolder } from "./useRevealInFolder.ts";
import { reviewedPaths } from "./reviewed-paths.ts";
import { AnnotationCard } from "#ui/routes/project/$id/workspace/AnnotationCard.tsx";
import { DiffThreadCard } from "#ui/routes/project/$id/workspace/DiffThreadCard.tsx";
import {
	type AnchoredThread,
	threadsByPathForScope,
	threadStillAnchored,
	type ThreadsByPath,
} from "#ui/review-threads.ts";
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
	hunkAddressIdentityKey,
	prepareDiffFiles,
	withoutFoldedHunks,
} from "./diff-view.ts";
import { DiffMinimap } from "./DiffMinimap.tsx";
import { ImageDiff } from "./ImageDiff.tsx";
import { DiffSearchBar } from "./DiffSearchBar.tsx";
import type { DiffSearchMatch } from "./diff-search.ts";
import { diffSearchMarksUnsafeCSS, useDiffSearchMarks } from "./diff-search-marks.ts";
import {
	getMinimapFiles,
	measureWrapColumns,
	type MinimapFile,
	type MinimapSelection,
} from "./diff-minimap.ts";
import {
	type ReviewedFileVersions,
	reviewedFilesQueryOptions,
	type SetFilesReviewedInput,
	useSetFilesReviewed,
} from "#ui/reviewed-files.ts";
import { useApplyToWorkspace } from "./useApplyToWorkspace.ts";
import { getRandomDadJoke } from "#ui/dad-jokes.ts";

export type DiffViewerHandle = CodeViewHandle<Annotation>;

// This must be unique as to not collide with other IDs, and stable because it's
// stored in local storage.
type PanelId = "files-panel" | "diff-panel";

const EMPTY_ANNOTATIONS_BY_PATH: LocalAnnotationsByPath = new Map();
const EMPTY_THREADS_BY_PATH: ThreadsByPath = new Map();
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
	commitDetails: CommitDetailsData;
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
	threadsByPath: ThreadsByPath,
): DiffView => ({
	...diffView,
	items: diffView.items.map((item) => {
		if (item.type === "file") return item;

		const file = diffView.fileByItemId.get(item.id);
		if (!file) throw new Error("Diff view file not found by ID");

		const persistedAnnotations = annotationsByPath.get(file.address.path) ?? [];
		const threads = threadsByPath.get(file.address.path) ?? [];
		if (persistedAnnotations.length === 0 && threads.length === 0) return item;

		const annotations: Array<DiffLineAnnotation<Annotation>> = [
			...persistedAnnotations.map(({ id, lineNumber, side }) => ({
				lineNumber,
				side,
				metadata: { _tag: "local" as const, id },
			})),
			...threads.map(({ thread, lineNumber, side }) => ({
				lineNumber,
				side,
				metadata: { _tag: "forge" as const, threadId: thread.id },
			})),
		];

		// Annotations move when their backend anchor drifts, so the version must cover their
		// positions and identities, not just their count. A thread also carries its reply
		// count, which is the one thing about it that changes in place.
		const annoHash = hash(
			[
				...persistedAnnotations.map((a) => `${a.id}:${a.side}:${a.lineNumber}`),
				...threads.map(
					(t) => `${t.thread.id}:${t.side}:${t.lineNumber}:${t.thread.comments.length}`,
				),
			].join(),
		);

		const version = item.version;
		if (version === undefined) throw new Error("Diff view item missing base version");

		return {
			...item,
			version: combineHashes(version, annoHash),
			annotations: [...(item.annotations ?? []), ...annotations],
		};
	}),
});

const navigationHunkForSelectedLines = ({
	selection,
	fileByItemId,
	hunkByKey,
}: {
	selection: CodeViewLineSelection | null;
	fileByItemId: DiffView["fileByItemId"];
	hunkByKey: DiffView["hunkByKey"];
}): HunkAddress | null => {
	if (!selection) return null;
	const file = fileByItemId.get(selection.id);
	if (file?.patch?.type !== "Patch") return null;

	const hunks = file.item.fileDiff.hunks;
	const side = selection.range.endSide ?? selection.range.side ?? "additions";
	const query = { hunks, line: selection.range.end, side };
	const wholeHunkSelection = wholeHunkSelectionByLine(query);
	const firstChangedLine = wholeHunkSelection?.lineGroups[0];
	const cursorSelection =
		contiguousSelectionByLine(query) ??
		(firstChangedLine
			? contiguousSelectionByLine({
					hunks,
					line: firstChangedLine.start,
					side: firstChangedLine.side,
				})
			: null);
	if (!cursorSelection) return null;

	const address: HunkAddress = {
		parent: file.address,
		...cursorSelection,
		isResultOfBinaryToTextConversion: file.patch.subject.isResultOfBinaryToTextConversion,
	};
	return hunkByKey.get(hunkAddressIdentityKey(address))?.address ?? null;
};

const lineSelectionsEqual = (a: CodeViewLineSelection, b: CodeViewLineSelection): boolean =>
	a.id === b.id &&
	a.range.start === b.range.start &&
	(a.range.side ?? "additions") === (b.range.side ?? "additions") &&
	a.range.end === b.range.end &&
	(a.range.endSide ?? a.range.side ?? "additions") ===
		(b.range.endSide ?? b.range.side ?? "additions");

const DadJokeFooter: FC = () => {
	const [{ setup, punchline }] = useState(getRandomDadJoke);

	return (
		<p className={styles.dadJoke}>
			<span>{setup}</span>
			<span>{punchline}</span>
		</p>
	);
};

const DiffContents: FC<{
	activeFileItemId: string | null;
	diffContextKey: string;
	focusScopeRef: RefObject<HTMLDivElement | null>;
	onViewerFileSelection: (path: string) => void;
	fileParent: FileParent;
	projectId: string;
	diffView: DiffView;
	annotationsByPath: LocalAnnotationsByPath;
	threadsByPath: ThreadsByPath;
	/** The review those threads hang on, which is how a reply is cached. */
	threadReviewId: number;
	diffBackgrounds?: GUISettings["diffBackground"];
	diffOverflow?: GUISettings["diffOverflow"];
	diffStyle?: GUISettings["diffStyle"];
	commentAnnotations: boolean;
	reviewedFiles: ReviewedFileVersions;
	manualCollapseByItem: Map<string, boolean>;
	setManualCollapse: (itemId: string, collapsed: boolean | undefined) => void;
	setFilesReviewed: (input: SetFilesReviewedInput) => void;
	viewerRef: RefObject<CodeViewHandle<Annotation> | null>;
	didScrollToViaFileRef: RefObject<boolean>;
	minimapFiles: Array<MinimapFile> | null;
	canUncommit: boolean;
	uncommit: (change: TreeChange, extendToCheckedFiles: boolean) => void;
}> = ({
	activeFileItemId,
	diffContextKey,
	focusScopeRef,
	onViewerFileSelection,
	fileParent,
	projectId,
	diffView: { items, addressSpace, hunkByKey, fileByItemId, fileByPath },
	annotationsByPath,
	threadsByPath,
	threadReviewId,
	diffBackgrounds,
	diffOverflow,
	diffStyle,
	commentAnnotations,
	reviewedFiles,
	manualCollapseByItem,
	setManualCollapse,
	setFilesReviewed,
	viewerRef,
	didScrollToViaFileRef,
	minimapFiles,
	canUncommit,
	uncommit,
}) => {
	const dispatch = useAppDispatch();
	const newFocusableAnnotationIdRef = useRef<string | null>(null);
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
	const revealInFolder = useRevealInFolder(projectId);
	const store = useAppStore();
	const queryClient = useQueryClient();
	const lineCheckRangeAnchor = useRef<string>(null);
	const lineCheckRangeEnd = useRef<string>(null);
	const hunkCheckRangeAnchor = useRef<string>(null);
	const hunkCheckRangeEnd = useRef<string>(null);

	const collapsedItems: Set<string> = new Set(
		items
			.values()
			.map((item) => {
				const manuallyCollapsed = manualCollapseByItem.get(item.id);
				if (manuallyCollapsed !== undefined) return manuallyCollapsed ? item.id : null;

				const file = fileByItemId.get(item.id);
				if (!file) return null;

				const {
					change: { path },
					item: { version },
				} = file;
				if (version === undefined) return null;

				const reviewedLatestVersion = reviewedFiles.get(path)?.has(version);
				return reviewedLatestVersion ? item.id : null;
			})
			.filter((x) => x != null),
	);
	const visibleAddressSpace = withoutFoldedHunks(addressSpace, hunkByKey, collapsedItems);

	const storedDiffSelection = useAppSelector((state) =>
		projectSlice.selectors.selectDiffCursor(state, projectId),
	);
	const storedSelectedLines = useMemo((): CodeViewLineSelection | null => {
		if (!storedDiffSelection) return null;
		const file = fileByItemId.get(weakFileIdentityKey(storedDiffSelection.file));
		return file ? { id: file.item.id, range: storedDiffSelection.range } : null;
	}, [storedDiffSelection, fileByItemId]);
	const storedSelectionHunk = useMemo(
		() =>
			navigationHunkForSelectedLines({
				selection: storedSelectedLines,
				fileByItemId,
				hunkByKey,
			}),
		[storedSelectedLines, fileByItemId, hunkByKey],
	);
	const diffSelection = storedSelectionHunk ?? visibleAddressSpace.items[0] ?? null;
	const hasStoredDiffSelection = storedDiffSelection !== null;
	const canCheckHunks = useAppSelector((state) =>
		projectSlice.selectors.selectCanCheckHunks(state, projectId, fileParent),
	);
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);
	const diffSelectionHunk =
		diffSelection !== null ? hunkByKey.get(hunkAddressIdentityKey(diffSelection)) : null;
	const cursorSelectedHunk = diffSelection
		? (hunkByKey.get(hunkAddressIdentityKey(diffSelection))?.selectedLines ?? null)
		: null;
	const cursorSelectedRange: CodeViewLineSelection | null = cursorSelectedHunk
		? {
				id: cursorSelectedHunk.id,
				range: {
					start: cursorSelectedHunk.range.start,
					side: cursorSelectedHunk.range.side,
					end: cursorSelectedHunk.range.start,
				},
			}
		: null;
	const selectedLines = storedSelectionHunk ? storedSelectedLines : cursorSelectedRange;

	const minimapSelection = useMemo((): MinimapSelection | null => {
		if (!selectedLines) return null;

		const { start, end, side, endSide } = selectedLines.range;
		return {
			itemId: selectedLines.id,
			side: side ?? "additions",
			start,
			endSide: endSide ?? side ?? "additions",
			end,
		};
	}, [selectedLines]);
	const selectedLinesHunk = storedSelectionHunk ?? diffSelection;
	const effectiveDiffStyle = diffStyle ?? defaultSettings.diffStyle;
	// Primitives, so the item list and header closures below only pick up new
	// identities when the selection crosses into another file — not on every
	// j/k move within one.
	const selectedFileItemId = diffSelectionHunk?.file.item.id ?? null;
	// Null while the selection sits on a visible hunk: only a folded file needs
	// its stand-in hunk rebuilt when it gains or loses the selection.
	const selectedFoldedFileId =
		selectedFileItemId != null && collapsedItems.has(selectedFileItemId)
			? selectedFileItemId
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

	const selectDiff = (selection: HunkAddress) => {
		const nextSelectedLines = hunkByKey.get(hunkAddressIdentityKey(selection))?.selectedLines;
		if (!nextSelectedLines) return;
		setCursor("diff", { file: selection.parent, range: nextSelectedLines.range });

		viewerRef.current?.scrollTo({
			type: "range",
			id: nextSelectedLines.id,
			range: nextSelectedLines.range,
			align: "nearest",
		});
	};
	const moveSelectedHunk = (offset: -1 | 1): void => {
		let selection = diffSelection;
		if (selectedLines) {
			const file = fileByItemId.get(selectedLines.id);
			if (file?.patch?.type === "Patch") {
				const fileHunks = file.hunks.map(({ address }) => address);
				const lineHunk = hunkSelectionForLineNavigation({
					hunks: file.item.fileDiff.hunks,
					selections: fileHunks,
					range: selectedLines.range,
					diffStyle: effectiveDiffStyle,
					offset,
				});
				selection = lineHunk ?? fileHunks.at(offset === 1 ? -1 : 0) ?? null;

				if (lineHunk) {
					const hunkLines = hunkByKey.get(hunkAddressIdentityKey(lineHunk))?.selectedLines;
					if (hunkLines && !lineSelectionsEqual(selectedLines, hunkLines)) {
						selectDiff(lineHunk);
						return;
					}
				}
			}
		}

		const next =
			selection === null
				? visibleAddressSpace.items.at(offset === 1 ? 0 : -1)
				: getAdjacent({
						addressSpace: visibleAddressSpace,
						selection,
						offset,
						getKey: hunkAddressIdentityKey,
					});
		if (next) selectDiff(next);
	};

	// A file's first hunk stands in for the file itself — a folded file keeps
	// only that one in the visible space, so it is always a reachable stop.
	const isFileStartHunk = (hunk: HunkAddress): boolean => {
		const key = hunkAddressIdentityKey(hunk);
		return (
			hunkAddressIdentityKey(assert(assert(hunkByKey.get(key)?.file.hunks[0])).address) === key
		);
	};

	// `selectDiff` only nudges the hunk into view; landing on a file should put
	// its header at the top, as picking the file in the file list does.
	const selectFileStart = (hunk: HunkAddress): void => {
		selectDiff(hunk);

		const itemId = hunkByKey.get(hunkAddressIdentityKey(hunk))?.file.item.id;
		if (itemId === undefined) return;
		viewerRef.current?.scrollTo({ type: "item", id: itemId, align: "start" });
	};

	const moveSelectedFile = (offset: -1 | 1): void => {
		const selection = selectedLinesHunk ?? diffSelection;
		if (selection === null) {
			const edge = visibleAddressSpace.items.at(offset === 1 ? 0 : -1);
			if (edge) selectFileStart(edge);
			return;
		}

		const selectionIndex = visibleAddressSpace.indexByKey.get(hunkAddressIdentityKey(selection));
		if (selectionIndex === undefined) return;

		// Going up from inside a file lands on that file's own start first, the
		// way section navigation does, so the key always feels like "one file".
		const startsOnFileStart = isFileStartHunk(selection);
		let index = selectionIndex + (offset === -1 && !startsOnFileStart ? 0 : offset);

		while (index >= 0 && index < visibleAddressSpace.items.length) {
			const hunk = visibleAddressSpace.items[index];
			if (hunk !== undefined && isFileStartHunk(hunk)) {
				selectFileStart(hunk);
				return;
			}
			index += offset;
		}
	};

	useAddressSpaceHotkeys({
		projectId,
		addressSpace: visibleAddressSpace,
		group: "Diff",
		select: selectDiff,
		selection: selectedLinesHunk ?? diffSelection,
		selectSectionPredicate: isFileStartHunk,
		ref: focusScopeRef,
		getKey: hunkAddressIdentityKey,
		operationSourcesForItem: (hunk) => {
			const selectedSources = addressesForSelectedLines(selectedLines, "compact");
			const sources = selectedSources.length > 0 ? selectedSources : [hunkAddress(hunk)];
			const checkedSources =
				selectedSources.length > 0 ? addressesForSelectedLines(selectedLines, "line") : sources;
			const state = store.getState();
			return checkedSources.every((source) =>
				projectSlice.selectors.selectAddressChecked(state, projectId, source),
			)
				? projectSlice.selectors.selectCheckedAddresses(state, projectId)
				: sources;
		},
		directionalNavigation: false,
	});

	const moveSelectedLines = (offset: -1 | 1, extend: boolean): void => {
		if (!selectedLines) return;
		const file = fileByItemId.get(selectedLines.id);
		if (!file || file.patch?.type !== "Patch") return;

		const range = moveSelectedLineRange({
			hunks: file.item.fileDiff.hunks,
			range: selectedLines.range,
			diffStyle: effectiveDiffStyle,
			offset,
			extend,
		});
		if (!range) return;

		const selection = { id: selectedLines.id, range };
		applySelectedLines(selection);
		viewerRef.current?.scrollTo({
			type: "range",
			id: selection.id,
			range,
			align: "nearest",
		});
	};

	function toggleSelectedLinesChecked(event: KeyboardEvent): void {
		if (event.composedPath().some(isInteractiveElement)) return;
		const addresses = addressesForSelectedLines(selectedLines, "line");
		if (addresses.length === 0) return;

		event.preventDefault();
		event.stopPropagation();
		const state = store.getState();
		const checked = !addresses.every((address) =>
			projectSlice.selectors.selectAddressChecked(state, projectId, address),
		);
		dispatch(projectSlice.actions.checkAddresses({ projectId, addresses, checked }));
	}

	const handleCreateComment = (
		line: Pick<DiffLineTarget, "itemId" | "lineNumber" | "side">,
	): void => {
		const file = fileByItemId.get(line.itemId);
		if (!file) return;

		const id = crypto.randomUUID();
		newFocusableAnnotationIdRef.current = id;

		createComment({
			projectId,
			comment: {
				id,
				path: file.address.path,
				commitChangeId: fileParent._tag === "Commit" ? fileParent.changeId : null,
				side: annotationSideToDiffSide(line.side),
				lineNumber: line.lineNumber,
				payload: "",
			},
		});
	};

	useHotkeys([
		{
			hotkey: "ArrowUp",
			callback: () => moveSelectedLines(-1, false),
			options: {
				conflictBehavior: "allow",
				enabled: selectedLines !== null,
				target: focusScopeRef,
			},
		},
		{
			hotkey: "K",
			callback: () => moveSelectedLines(-1, false),
			options: {
				conflictBehavior: "allow",
				enabled: selectedLines !== null,
				target: focusScopeRef,
			},
		},
		{
			hotkey: "ArrowDown",
			callback: () => moveSelectedLines(1, false),
			options: {
				conflictBehavior: "allow",
				enabled: selectedLines !== null,
				target: focusScopeRef,
			},
		},
		{
			hotkey: "J",
			callback: () => moveSelectedLines(1, false),
			options: {
				conflictBehavior: "allow",
				enabled: selectedLines !== null,
				target: focusScopeRef,
			},
		},
		{
			hotkey: "Shift+ArrowUp",
			callback: () => moveSelectedLines(-1, true),
			options: {
				conflictBehavior: "allow",
				enabled: selectedLines !== null,
				target: focusScopeRef,
			},
		},
		{
			hotkey: "Shift+K",
			callback: () => moveSelectedLines(-1, true),
			options: {
				conflictBehavior: "allow",
				enabled: selectedLines !== null,
				target: focusScopeRef,
			},
		},
		{
			hotkey: "Shift+ArrowDown",
			callback: () => moveSelectedLines(1, true),
			options: {
				conflictBehavior: "allow",
				enabled: selectedLines !== null,
				target: focusScopeRef,
			},
		},
		{
			hotkey: "Shift+J",
			callback: () => moveSelectedLines(1, true),
			options: {
				conflictBehavior: "allow",
				enabled: selectedLines !== null,
				target: focusScopeRef,
			},
		},
		{
			hotkey: "Alt+ArrowUp",
			callback: () => moveSelectedHunk(-1),
			options: {
				conflictBehavior: "allow",
				target: focusScopeRef,
			},
		},
		{
			hotkey: "Alt+K",
			callback: () => moveSelectedHunk(-1),
			options: {
				conflictBehavior: "allow",
				target: focusScopeRef,
			},
		},
		{
			hotkey: "Alt+ArrowDown",
			callback: () => moveSelectedHunk(1),
			options: {
				conflictBehavior: "allow",
				target: focusScopeRef,
			},
		},
		{
			hotkey: "Alt+J",
			callback: () => moveSelectedHunk(1),
			options: {
				conflictBehavior: "allow",
				target: focusScopeRef,
			},
		},
		{
			hotkey: diffHotkeys.previousFile.hotkey,
			callback: () => moveSelectedFile(-1),
			options: {
				conflictBehavior: "allow",
				target: focusScopeRef,
				meta: diffHotkeys.previousFile.meta,
			},
		},
		{
			hotkey: "Alt+Shift+K",
			callback: () => moveSelectedFile(-1),
			options: {
				conflictBehavior: "allow",
				target: focusScopeRef,
			},
		},
		{
			hotkey: diffHotkeys.nextFile.hotkey,
			callback: () => moveSelectedFile(1),
			options: {
				conflictBehavior: "allow",
				target: focusScopeRef,
				meta: diffHotkeys.nextFile.meta,
			},
		},
		{
			hotkey: "Alt+Shift+J",
			callback: () => moveSelectedFile(1),
			options: {
				conflictBehavior: "allow",
				target: focusScopeRef,
			},
		},
		{
			hotkey: diffHotkeys.absorb.hotkey,
			callback: () => {
				if (!diffSelectionHunk) return;
				const firstLine = diffSelectionHunk.address.lineGroups[0];
				if (!firstLine) return;

				const hunk = getHunkAddressAtLine({
					itemId: diffSelectionHunk.file.item.id,
					lineNumber: firstLine.start,
					side: firstLine.side,
					lineType: "change",
				});
				if (!hunk) return;

				startAbsorb({
					sources: [hunkAddress(hunk)],
					sourceTarget: {
						type: "hunks",
						subject: {
							hunks: [
								{
									pathBytes: diffSelectionHunk.file.change.pathBytes,
									hunkHeader: hunk.hunkHeader,
								},
							],
						},
					},
				});

				focusScope("sidebar");
			},
			options: {
				enabled:
					fileParent._tag === "UncommittedChanges" &&
					noOperationPending &&
					!!diffSelectionHunk &&
					!diffSelectionHunk.address.isResultOfBinaryToTextConversion,
				conflictBehavior: "allow",
				target: focusScopeRef,
				meta: diffHotkeys.absorb.meta,
			},
		},
		{
			hotkey: diffHotkeys.addComment.hotkey,
			callback: () => {
				if (!selectedLines) return;

				handleCreateComment({
					itemId: selectedLines.id,
					lineNumber: selectedLines.range.end,
					side: selectedLines.range.endSide ?? selectedLines.range.side ?? "additions",
				});
			},
			options: {
				enabled: fileParent._tag !== "Branch" && noOperationPending && selectedLines !== null,
				conflictBehavior: "allow",
				target: focusScopeRef,
				meta: diffHotkeys.addComment.meta,
			},
		},
		{
			hotkey: diffHotkeys.checkHunk.hotkey,
			callback: toggleSelectedLinesChecked,
			options: {
				conflictBehavior: "allow",
				enabled: selectedLines !== null && canCheckHunks,
				preventDefault: false,
				stopPropagation: false,
				target: focusScopeRef,
				meta: diffHotkeys.checkHunk.meta,
			},
		},
		{
			hotkey: "Shift+Space",
			callback: toggleSelectedLinesChecked,
			options: {
				conflictBehavior: "allow",
				enabled: selectedLines !== null && canCheckHunks,
				preventDefault: false,
				stopPropagation: false,
				target: focusScopeRef,
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
				target: focusScopeRef,
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
				target: focusScopeRef,
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
					lineNr: selectedLines?.range.start ?? null,
				}),
			options: {
				enabled: !!diffSelectionHunk && !!settings?.editor,
				conflictBehavior: "allow",
				target: focusScopeRef,
				meta: diffHotkeys.openInEditor.meta,
			},
		},
		{
			hotkey: diffHotkeys.revealInFolder.hotkey,
			callback: () => {
				if (!diffSelectionHunk) return;
				void revealInFolder(diffSelectionHunk.file.change.path);
			},
			options: {
				enabled: !!diffSelectionHunk,
				conflictBehavior: "allow",
				target: focusScopeRef,
				meta: diffHotkeys.revealInFolder.meta,
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

		onViewerFileSelection(file.address.path);
	};

	function addressForLineSelection(
		itemId: string,
		selection: HunkLineSelection | null,
	): HunkAddress | null {
		if (!selection) return null;
		const file = fileByItemId.get(itemId);
		if (file?.patch?.type !== "Patch") return null;

		return {
			parent: { parent: fileParent, path: file.change.path },
			...selection,
			isResultOfBinaryToTextConversion: file.patch.subject.isResultOfBinaryToTextConversion,
		};
	}

	function addressesForSelectedLines(
		selection: CodeViewLineSelection | null,
		granularity: "compact" | "line",
	): Array<Extract<Address, { _tag: "Hunk" }>> {
		if (!selection) return [];
		const file = fileByItemId.get(selection.id);
		if (file?.patch?.type !== "Patch") return [];
		const isResultOfBinaryToTextConversion = file.patch.subject.isResultOfBinaryToTextConversion;

		return lineSelectionsForRange({
			hunks: file.item.fileDiff.hunks,
			range: selection.range,
			diffStyle: effectiveDiffStyle,
			granularity,
		}).map((lineSelection) =>
			hunkAddress({
				parent: { parent: fileParent, path: file.change.path },
				...lineSelection,
				isResultOfBinaryToTextConversion,
			}),
		);
	}

	function applySelectedLines(selection: CodeViewLineSelection | null): void {
		if (!selection) return setCursor("diff", null);
		const file = fileByItemId.get(selection.id);
		if (!file) return;
		setCursor("diff", { file: file.address, range: selection.range });
	}

	const handleLinesSelected = (selection: CodeViewLineSelection | null): void => {
		// Keep the active line selected when it is clicked again: Lite treats line selection as a
		// persistent operation target, not a toggle. Still clear it when its item leaves the view.
		if (selection === null && selectedLines !== null && fileByItemId.has(selectedLines.id)) return;

		applySelectedLines(selection);
	};

	const getLineAddressAtLine = ({
		itemId,
		lineNumber,
		side,
	}: DiffLineTarget): HunkAddress | null => {
		const file = fileByItemId.get(itemId);
		if (file?.patch?.type !== "Patch") return null;

		return addressForLineSelection(
			itemId,
			singleLineSelectionByLine({ hunks: file.item.fileDiff.hunks, line: lineNumber, side }),
		);
	};

	const getHunkAddressAtLine = ({
		itemId,
		lineNumber,
		side,
	}: DiffLineTarget): HunkAddress | null => {
		const file = fileByItemId.get(itemId);
		if (file?.patch?.type !== "Patch") return null;

		return addressForLineSelection(
			itemId,
			wholeHunkSelectionByLine({ hunks: file.item.fileDiff.hunks, line: lineNumber, side }),
		);
	};

	// Keep this option's identity fixed while reading the latest diff state. An inline callback (or
	// useCallback with the render-local helpers as dependencies) invalidates the compiler's cached
	// CodeView on focus, causing Pierre to rebuild its DOM during native text selection.
	const handleLineNumberClick: NonNullable<CodeViewOptions<Annotation>["onLineNumberClick"]> =
		useStableCallback(({ event, numberElement }, context) => {
			if (event.detail !== 2) return;
			const target = diffLineTargetFromElement({
				element: numberElement,
				itemId: context.item.id,
			});
			if (!target) return;
			const address = getHunkAddressAtLine(target);
			if (!address) return;
			const range = rangeFromLineGroups(address.lineGroups);
			if (!range) return;

			applySelectedLines({ id: target.itemId, range });
		});

	const getContiguousHunkAddressAtLine = ({
		itemId,
		lineNumber,
		side,
	}: DiffLineTarget): HunkAddress | null => {
		const file = fileByItemId.get(itemId);
		if (file?.patch?.type !== "Patch") return null;

		return addressForLineSelection(
			itemId,
			contiguousSelectionByLine({ hunks: file.item.fileDiff.hunks, line: lineNumber, side }),
		);
	};

	const getContextMenuAddressAtLine = ({
		itemId,
		lineNumber,
		side,
		lineType,
	}: DiffLineTarget): HunkAddress | null => {
		const file = fileByItemId.get(itemId);
		if (file?.patch?.type !== "Patch") return null;
		const query = { hunks: file.item.fileDiff.hunks, line: lineNumber, side };

		return addressForLineSelection(
			itemId,
			lineType === "context" ? wholeHunkSelectionByLine(query) : contiguousSelectionByLine(query),
		);
	};

	const checkedHunkKeys = (): Set<string> =>
		new Set(
			projectSlice.selectors
				.selectCheckedAddresses(store.getState(), projectId)
				.values()
				.map((address) => (address._tag === "Hunk" ? hunkAddressIdentityKey(address) : null))
				.filter((x) => x != null),
		);

	const applyCheckedAddressGroups = ({
		previous,
		next,
		addressesByKey,
	}: {
		previous: Set<string>;
		next: Set<string>;
		addressesByKey: Map<string, Array<Extract<Address, { _tag: "Hunk" }>>>;
	}): void => {
		const addressesForKeys = (keys: Set<string>): Array<CheckableAddress> =>
			keys
				.values()
				.flatMap((key) => addressesByKey.get(key) ?? [])
				.toArray();

		dispatch(
			projectSlice.actions.checkAddresses({
				projectId,
				addresses: addressesForKeys(next.difference(previous)),
				checked: true,
			}),
		);
		dispatch(
			projectSlice.actions.checkAddresses({
				projectId,
				addresses: addressesForKeys(previous.difference(next)),
				checked: false,
			}),
		);
	};

	const checkedRangeFor = ({
		orderedAddresses,
		checked,
		rangeAnchor,
		rangeEnd,
		target,
		shiftKey,
	}: {
		orderedAddresses: Array<HunkAddress>;
		checked: Set<string>;
		rangeAnchor: string | null;
		rangeEnd: string | null;
		target: HunkAddress;
		shiftKey: boolean;
	}) => {
		const addressSpace = {
			items: orderedAddresses,
			indexByKey: buildIndexByKey(orderedAddresses, hunkAddressIdentityKey),
		};
		const resolveRange = addressSpaceRange({
			addressSpace,
			getKey: (key: string) => key,
			filterMap: (address: HunkAddress) => hunkAddressIdentityKey(address),
		});

		return checkedRange(resolveRange)({ checked, rangeAnchor, rangeEnd })({
			item: hunkAddressIdentityKey(target),
			shiftKey,
		});
	};

	const visibleHunkGroups = () =>
		visibleAddressSpace.items
			.values()
			.map((address) => {
				const selection = hunkByKey.get(hunkAddressIdentityKey(address))?.selectedLines;
				const lineAddresses = selection ? addressesForSelectedLines(selection, "line") : null;
				return lineAddresses && lineAddresses.length > 0 ? { address, lineAddresses } : null;
			})
			.filter((x) => x != null);

	// Checkbox Shift-click extends persistent checked ranges. Shift-clicking the surrounding gutter
	// remains Pierre's active line-range gesture, unlike the whole-row shortcut on file/commit rows.
	function checkLine(address: HunkAddress, shiftKey: boolean): void {
		const key = hunkAddressIdentityKey(address);
		const previous = shiftKey && lineCheckRangeAnchor.current !== null ? checkedHunkKeys() : null;
		if (previous && previous.size > 0) {
			const orderedAddresses = visibleHunkGroups()
				.flatMap(({ lineAddresses }) => lineAddresses)
				.toArray();
			const addressesByKey = new Map(
				orderedAddresses.map((lineAddress) => [hunkAddressIdentityKey(lineAddress), [lineAddress]]),
			);
			const nextRange = checkedRangeFor({
				orderedAddresses,
				checked: previous,
				rangeAnchor: lineCheckRangeAnchor.current,
				rangeEnd: lineCheckRangeEnd.current,
				target: address,
				shiftKey,
			});

			lineCheckRangeAnchor.current = nextRange.rangeAnchor;
			lineCheckRangeEnd.current = nextRange.rangeEnd;
			return applyCheckedAddressGroups({
				previous,
				next: nextRange.checked,
				addressesByKey,
			});
		}

		const source = hunkAddress(address);
		const checked = !projectSlice.selectors.selectAddressChecked(
			store.getState(),
			projectId,
			source,
		);
		lineCheckRangeAnchor.current = key;
		lineCheckRangeEnd.current = key;
		dispatch(projectSlice.actions.checkAddress({ projectId, address: source, checked }));
	}

	function checkHunkLines(
		address: HunkAddress,
		lineAddresses: Array<Extract<Address, { _tag: "Hunk" }>>,
		shiftKey: boolean,
	): void {
		const key = hunkAddressIdentityKey(address);
		if (!shiftKey || hunkCheckRangeAnchor.current === null) {
			const state = store.getState();
			const checked = !lineAddresses.every((lineAddress) =>
				projectSlice.selectors.selectAddressChecked(state, projectId, lineAddress),
			);
			hunkCheckRangeAnchor.current = key;
			hunkCheckRangeEnd.current = key;
			dispatch(
				projectSlice.actions.checkAddresses({ projectId, addresses: lineAddresses, checked }),
			);
			return;
		}

		const groups = visibleHunkGroups().toArray();
		const addressesByKey = new Map(
			groups.map(({ address, lineAddresses }) => [hunkAddressIdentityKey(address), lineAddresses]),
		);
		const state = store.getState();
		const previous = new Set(
			groups
				.values()
				.map(({ address, lineAddresses }) =>
					lineAddresses.every((lineAddress) =>
						projectSlice.selectors.selectAddressChecked(state, projectId, lineAddress),
					)
						? hunkAddressIdentityKey(address)
						: null,
				)
				.filter((x) => x != null),
		);
		const nextRange = checkedRangeFor({
			orderedAddresses: groups.map(({ address }) => address),
			checked: previous,
			rangeAnchor: hunkCheckRangeAnchor.current,
			rangeEnd: hunkCheckRangeEnd.current,
			target: address,
			shiftKey,
		});

		hunkCheckRangeAnchor.current = nextRange.rangeAnchor;
		hunkCheckRangeEnd.current = nextRange.rangeEnd;
		applyCheckedAddressGroups({ previous, next: nextRange.checked, addressesByKey });
	}

	const handleLineContextMenu = ({ event, ...target }: DiffLineContextMenuTarget): void => {
		const file = fileByItemId.get(target.itemId);
		if (!file) return;
		const address = getContextMenuAddressAtLine(target);
		if (!address) return;
		const hunk = getHunkAddressAtLine(target);
		if (!hunk) return;
		const lineAddress = getLineAddressAtLine(target);
		const checkedProbe = lineAddress ? hunkAddress(lineAddress) : null;
		const selectedAddresses = addressesForSelectedLines(selectedLines, "compact");
		const usesSelectedLines =
			selectedLines?.id === target.itemId &&
			selectedAddresses.length > 0 &&
			selectedLineRangeContainsPoint({
				hunks: file.item.fileDiff.hunks,
				range: selectedLines.range,
				diffStyle: effectiveDiffStyle,
				line: target.lineNumber,
				side: target.side,
			});

		void showNativeContextMenu(
			event,
			hunkMenuItems({
				change: file.change,
				hunk,
				lineNumber: target.lineNumber,
				sources: usesSelectedLines ? selectedAddresses : [hunkAddress(address)],
				checkedProbe,
				usesSelectedLines,
			}),
		);
	};

	useDiffLineContextMenu({
		viewerRef,
		onContextMenu: handleLineContextMenu,
	});

	const handleHunkPostRender = useDiffHunkDrag<Annotation>({
		projectId,
		fileParent,
		getHunkAddress: getHunkAddressAtLine,
		getLineAddress: getLineAddressAtLine,
		getSelectedAddresses: () => addressesForSelectedLines(selectedLines, "compact"),
	});
	const { onPostRender: handleDiffPostRender, portals: diffGutterPortals } =
		useDiffGutterCheckboxes(
			handleHunkPostRender,
			getLineAddressAtLine,
			getContiguousHunkAddressAtLine,
			projectId,
			checkLine,
			checkHunkLines,
			commentAnnotations && fileParent._tag !== "Branch" ? handleCreateComment : undefined,
		);
	const {
		onPostRender: handleMarkedDiffPostRender,
		setSearchMatches,
		getSearchSource,
		searchMarks,
	} = useDiffSearchMarks(handleDiffPostRender, items);

	const handOffCollapsedSelection = (itemId: string): void => {
		// Folding hides the selected hunk's lines; hand the selection to the
		// file's first hunk, which stands in for the folded file, and keep the
		// header in view. The stored selection is read off the store rather than
		// captured, so this callback's identity does not churn with j/k moves.
		const stored = projectSlice.selectors.selectDiffCursor(store.getState(), projectId);
		const storedFile = stored && fileByItemId.get(weakFileIdentityKey(stored.file));
		if (storedFile?.item.id !== itemId) return;

		selectDiff(assert(storedFile.hunks[0]).address);
		viewerRef.current?.scrollTo({ type: "item", id: itemId, align: "nearest" });
	};

	const handleSetCollapsed = (itemId: string) => (collapsed: boolean) => {
		setManualCollapse(itemId, collapsed);
		if (collapsed && !collapsedItems.has(itemId)) handOffCollapsedSelection(itemId);
	};

	// Stable so typing in the search bar only re-renders the bar, never this
	// component; the callback still reads the render-fresh helpers it closes over.
	const navigateToSearchMatch = useStableCallback((match: DiffSearchMatch): void => {
		if (collapsedItems.has(match.itemId)) setManualCollapse(match.itemId, false);

		applySelectedLines({
			id: match.itemId,
			range: {
				start: match.lineNumber,
				side: match.side,
				end: match.lineNumber,
				endSide: match.side,
			},
		});

		// A frame later, so an unfold above reaches CodeView's layout before the
		// scroll asks it where the line is.
		requestAnimationFrame(() => {
			viewerRef.current?.scrollTo({
				type: "line",
				id: match.itemId,
				lineNumber: match.lineNumber,
				side: match.side,
				align: "center",
			});
		});
	});

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
	const enhanceCollapsed = <T,>(item: CodeViewItem<T>, selected: boolean): CodeViewItem<T> => ({
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

	const loadDiffFiles: NonNullable<CodeViewOptions<Annotation>["loadDiffFiles"]> =
		useStableCallback(async (fileDiff) => {
			const file = fileByPath.get(fileDiff.name);
			if (file?.patch?.type !== "Patch") throw new Error("Cannot expand non-patch diff");
			if (file.patch.subject.isResultOfBinaryToTextConversion)
				throw new Error("Cannot expand text-converted diff");

			const { version } = file.item;
			if (version === undefined) throw new Error("Diff view item missing version");

			const { change } = file;
			if (change.status.type !== "Modification" && change.status.type !== "Rename")
				throw new Error(`Cannot load full files for ${fileDiff.name}`);

			const loadBlobFile = async (path: string, blobId: string): Promise<FileContents> => {
				const res = await queryClient.fetchQuery(
					blobFileQueryOptions({ projectId, relativePath: path, blobId }),
				);
				if (res.content === null || res.mimeType !== null)
					throw new Error("Could not load file contents from blob");

				return { name: path, contents: res.content, cacheKey: blobId };
			};

			const loadWorkspaceFile = async (path: string): Promise<FileContents> => {
				const res = await queryClient.fetchQuery(
					workspaceFileQueryOptions({ projectId, relativePath: path, version }),
				);
				if (res.content === null || res.mimeType !== null)
					throw new Error("Could not load file contents from workspace");

				return {
					name: path,
					contents: res.content,
					cacheKey: `workspace:${path}:${version}`,
				};
			};

			// Don't await yet, retain prospective parallelisation.
			const asyncNewFile =
				fileParent._tag === "UncommittedChanges"
					? loadWorkspaceFile(change.path)
					: loadBlobFile(change.path, change.status.subject.state.id);

			if (fileDiff.type === "rename-pure") return { oldFile: null, newFile: await asyncNewFile };

			const [oldFile, newFile] = await Promise.all([
				loadBlobFile(
					change.status.type === "Rename" ? change.status.subject.previousPath : change.path,
					change.status.subject.previousState.id,
				),
				asyncNewFile,
			]);
			return { oldFile, newFile };
		});

	// `Diff` short-circuits the whole tab before this renders, so an empty item
	// list here is a frame between renders rather than a state to describe.
	return items.length === 0 ? null : (
		<>
			<CodeView
				ref={viewerRef}
				renderCodeViewFooter={() => <DadJokeFooter key={diffContextKey} />}
				renderCustomHeader={(item) => {
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
							address={file.address}
							change={file.change}
							hasDiff={item.type === "file" || file.item.fileDiff.hunks.length !== 0}
							collapsed={item.collapsed ?? false}
							reviewState={reviewState}
							lineStats={patchLineStats(file.patch)}
							selected={item.id === selectedFileItemId}
							setCollapsed={handleSetCollapsed(item.id)}
							setReviewed={handleSetReviewed(item.id, file.change.path, version)}
							canUncommit={canUncommit}
							uncommit={uncommit}
						/>
					);
				}}
				renderAnnotation={(anno, item) => {
					if (anno.metadata._tag === "image") {
						const file = fileByItemId.get(item.id);
						if (!file) return null;

						return (
							<ImageDiff
								projectId={projectId}
								change={file.change}
								fileParent={fileParent}
								version={file.item.version ?? 0}
							/>
						);
					}

					if (anno.metadata._tag === "forge") {
						const file = fileByItemId.get(item.id);
						if (!file) return null;

						const threadId = anno.metadata.threadId;
						const anchored = (threadsByPath.get(file.address.path) ?? []).find(
							({ thread }) => thread.id === threadId,
						);
						if (!anchored) return null;

						return (
							<DiffThreadCard
								projectId={projectId}
								reviewId={threadReviewId}
								thread={anchored.thread}
							/>
						);
					}

					if (!isDiffAnnotation<Annotation>(anno)) return null;

					const file = fileByItemId.get(item.id);
					if (!file) return null;

					const annotations = annotationsByPath.get(file.address.path) ?? [];
					const annotationId = anno.metadata.id;
					const annotation = annotations.find(({ id }) => id === annotationId);
					if (!annotation) return null;

					return (
						<AnnotationCard
							projectId={projectId}
							annotation={annotation}
							path={file.address.path}
							fileParent={fileParent}
							annotationsByPath={annotationsByPath}
							focusAnnotationIdRef={newFocusableAnnotationIdRef}
							focusScopeRef={focusScopeRef}
						/>
					);
				}}
				onScroll={selectFileAtViewportTop}
				className={styles.diffContents}
				items={displayItems}
				selectedLines={selectedLines}
				onSelectedLinesChange={handleLinesSelected}
				options={{
					diffStyle: effectiveDiffStyle,
					loadDiffFiles,
					disableBackground: !(diffBackgrounds ?? defaultSettings.diffBackground),
					lineDiffType: settings?.lineDiffType ?? defaultSettings.lineDiffType,
					overflow: diffOverflow ?? defaultSettings.diffOverflow,
					themeType: settings?.theme ?? defaultSettings.theme,
					stickyHeaders: true,
					enableLineSelection: true,
					onLineNumberClick: handleLineNumberClick,
					layout: codeViewLayout,
					// This appears to validate before our custom header has been slotted, in which case - if
					// our metrics are correct - we should see deltas in multiples of our custom header height
					// as defined in the metrics. We'll see an additional set of logs if there are other issues
					// with our metrics.
					__devOnlyValidateItemHeights: false,
					onPostRender: handleMarkedDiffPostRender,
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

    		  /* Pierre doesn't support image diffs yet:
               https://github.com/pierrecomputer/pierre/issues/258

             We leverage annotations on synthetic empty diffs as a workaround. Here we hide the
   		       empty diff that causes to render. */
    		  pre[data-file] {
    		    user-select: none;
    		  }

    		  pre[data-file] :is([data-column-number], [data-line]) {
    		    visibility: hidden;
    		    pointer-events: none;
    		  }

          [data-column-number] {
            --mix-selection-light: 0%;
            --mix-selection-dark: 0%;

            cursor: default;
          }

          [data-column-number][data-selected-line]:is(
            [data-line-type="context"],
            [data-line-type="context-expanded"]
          ) {
            --diffs-bg-selection-number-override: color-mix(
              var(--fill-gray-bg) var(--opacity-bg-selected-blur),
              transparent
            );

            color: var(--text-1);
          }

          /* Pierre pins the leading hunk separator flush against the file header:
             its virtual layout models no gap before the first separator, so a real
             margin would desync item heights. Inset the band inside the row
             instead — same box, a little air under the header. */
          [data-separator="line-info"][data-separator-first] {
            background-color: transparent;

            & [data-separator-wrapper] {
              top: 6px;
              height: calc(100% - 6px);
            }
          }

          ${diffGutterUnsafeCSS}
          ${diffSearchMarksUnsafeCSS}
        `,
				}}
				style={{
					"--diffs-font-family": settings?.diffFontFamily ?? defaultSettings.diffFontFamily,
					"--diffs-font-size": `${settings?.diffFontSize ?? defaultSettings.diffFontSize}px`,
					"--diffs-tab-size": `${settings?.diffTabSize ?? defaultSettings.diffTabSize}`,
					"--gitbutler-diff-gutter-can-drag": String(fileParent._tag !== "Branch"),
				}}
			/>

			{diffGutterPortals}

			<DiffSearchBar
				items={items}
				getSearchSource={getSearchSource}
				focusScopeRef={focusScopeRef}
				onNavigate={navigateToSearchMatch}
				onMatchesChange={setSearchMatches}
			/>

			{minimapFiles && (
				<DiffMinimap
					viewerRef={viewerRef}
					files={minimapFiles}
					diffStyle={effectiveDiffStyle}
					annotationsByPath={annotationsByPath}
					threadsByPath={threadsByPath}
					selection={minimapSelection}
					searchMarks={searchMarks}
				/>
			)}
		</>
	);
};

type DiffFileHeaderProps = {
	projectId: string;
	address: FileAddress;
	change: TreeChange;
	hasDiff: boolean;
	collapsed: boolean;
	reviewState: "reviewed" | "changed" | null;
	/** The change's counted deltas, or `null` when there is no patch to count. */
	lineStats: LineStats | null;
	/** Whether the diff selection sits in this file. */
	selected: boolean;
	setCollapsed: (collapsed: boolean) => void;
	setReviewed: (reviewed: boolean) => void;
	canUncommit: boolean;
	uncommit: (change: TreeChange, extendToCheckedFiles: boolean) => void;
};

const DiffFileHeader: FC<DiffFileHeaderProps> = (p) => {
	const menuItems = useFileMenuItems({
		projectId: p.projectId,
		address: p.address,
		path: p.change.path,
		change: p.change,
		canUncommit: p.canUncommit,
		uncommit: p.uncommit,
	});

	const lastSepIdx = p.change.path.lastIndexOf("/");
	const directoryPath = lastSepIdx !== -1 ? p.change.path.slice(0, lastSepIdx) : null;
	const fileName = lastSepIdx !== -1 ? p.change.path.slice(lastSepIdx + 1) : p.change.path;

	// The counts read as added/removed lines on sight, but only to someone who
	// knows the colouring: the wording carries the units, for the tooltip and for
	// screen readers alike.
	const lineStatsParts = p.lineStats === null ? [] : describeLineStats(p.lineStats);
	const lineStatsLabel = lineStatsParts.length === 0 ? null : lineStatsParts.join(", ");

	const collapseLabel = p.collapsed ? "Unfold" : "Fold";
	const reviewLabel =
		p.reviewState === "reviewed"
			? "Reviewed"
			: p.reviewState === "changed"
				? "Needs review"
				: "Not reviewed";

	return (
		<OperationSourceC
			projectId={p.projectId}
			sources={[fileAddress(p.address)]}
			respectChecked={false}
			outline="inside"
			acceptOriginDrop
		>
			<header
				// Not a tab stop, but mouse-focusable: clicking the header's own chrome
				// focuses it as the nearest focusable ancestor, so Tab walks this file's
				// actions instead of restarting at the first file in the diff, which is
				// where focus landing on the diff container sends it.
				tabIndex={-1}
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
				<div className={styles.fileHeaderEnd}>
					{p.lineStats && lineStatsLabel !== null && (
						<Tooltip.Root>
							<Tooltip.Trigger
								render={
									<div aria-label={lineStatsLabel} className={styles.fileMeta}>
										<DiffStats
											added={p.lineStats.linesAdded}
											removed={p.lineStats.linesRemoved}
											className="text-12"
										/>
										<ChangeScale
											added={p.lineStats.linesAdded}
											removed={p.lineStats.linesRemoved}
										/>
									</div>
								}
							/>
							<Tooltip.Portal>
								<Tooltip.Positioner sideOffset={4}>
									<Tooltip.Popup render={<TooltipPopup />}>{lineStatsLabel}</Tooltip.Popup>
								</Tooltip.Positioner>
							</Tooltip.Portal>
						</Tooltip.Root>
					)}

					<Toolbar.Root aria-label="File actions" className={styles.fileHeaderActions}>
						<Toolbar.Separator className={styles.fileHeaderSeparator} />
						{/* One button carrying checkbox semantics, with the box drawn inside it,
						    rather than a real Checkbox nested in a button or a label. Both of
						    those leave two controls where the design has one, and Base UI's
						    checkbox renders unfocusable inside a label. "Changed since you
						    reviewed it" is the mixed state; the tooltip spells that out. */}
						<Tooltip.Root>
							<Tooltip.Trigger
								render={
									<Toolbar.Button
										aria-pressed={
											p.reviewState === "changed" ? "mixed" : p.reviewState === "reviewed"
										}
										className={classes(
											getButtonClassName({ size: "small", variant: "ghost" }),
											styles.fileReview,
										)}
										onClick={() => p.setReviewed(p.reviewState !== "reviewed")}
									>
										<span className={styles.fileReviewBox} aria-hidden="true">
											{p.reviewState !== null && (
												<Icon size={10} name={p.reviewState === "reviewed" ? "tick" : "minus"} />
											)}
										</span>
										Reviewed
									</Toolbar.Button>
								}
							/>
							<Tooltip.Portal>
								<Tooltip.Positioner sideOffset={4}>
									<Tooltip.Popup render={<TooltipPopup />}>{reviewLabel}</Tooltip.Popup>
								</Tooltip.Positioner>
							</Tooltip.Portal>
						</Tooltip.Root>
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
				</div>
			</header>
		</OperationSourceC>
	);
};

const FilesToggle: FC<{ projectId: string }> = ({ projectId }) => {
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
 * its inputs; derived in render, the rows — and the address space built from
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
	onActiveFileSelection: (itemId: string, firstSelection: DiffLineSelection | null) => void;
	onPassiveFileSelection: (selection: string) => void;
	selection: Address;
	projectId: string;
	viewerRef: RefObject<DiffViewerHandle | null>;
	didScrollToViaFileRef: RefObject<boolean>;
	headerSlot?: ReactNode;
	/**
	 * Whether this scope may have a files panel at all. Its caller knows, and the
	 * URL no longer does: the pane can be driven by a list the `active` param is
	 * not naming.
	 */
	canShowFiles: boolean;
}> = ({
	changes: unsortedChanges,
	filesVisible,
	canShowFiles,
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
	const focusScopeRef = useRef<HTMLDivElement>(null);
	const store = useAppStore();
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

	const {
		data: { unidiff: renderAllFiles, commentAnnotations },
	} = useSuspenseQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => ({
			commentAnnotations: cfg.commentAnnotations ?? defaultSettings.commentAnnotations,
			unidiff: cfg.unidiff ?? defaultSettings.unidiff,
		}),
	});

	const detailsFullWindow = useAppSelector(interfaceSlice.selectors.selectDetailsFullWindow);

	// Change stats live in the files panel, or — in the uncommitted scope, which has no files
	// panel — in the sidebar's "Uncommitted" row. Surface them in the toolbar below whenever
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
	// address space are keyed on — so it is memoised by hand.
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
	const filesAddressSpace = useMemo(() => fileTreeAddressSpace(filesRows), [filesRows]);
	const filesSelection = useSelection("files", filesAddressSpace);

	// At time of writing React Compiler cannot statically analyse that these are pure derivations of
	// the sidebar selection, even with the helpers inlined, hence manual memoisation.
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

	const { isPending: isCommitUncommitChangesPending, mutate: commitUncommitChanges } =
		useCommitUncommitChanges();

	const uncommit = (change: TreeChange, extendToCheckedFiles: boolean): void => {
		if (fileParent._tag !== "Commit") return;

		const sources = projectSlice.selectors.selectCheckedAddresses(store.getState(), projectId);

		let subjectChanges = [change];
		if (
			extendToCheckedFiles &&
			sources.length > 0 &&
			sources.every(
				(address) => address._tag === "File" && addressEquals(address.parent, fileParent),
			)
		) {
			const checkedChanges = sources
				.values()
				.map((source) =>
					changes.find((candidate) => source._tag === "File" && candidate.path === source.path),
				)
				.filter((x) => x != null)
				.toArray();
			if (checkedChanges.length !== sources.length) return;

			subjectChanges = checkedChanges;
		}

		commitUncommitChanges({
			projectId,
			commitId: fileParent.commitId,
			assignTo: null,
			changes: subjectChanges.map((change) => createDiffSpec(change, [])),
			dryRun: false,
		});
	};
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

	const { data: loadedAnnotationsByPath = EMPTY_ANNOTATIONS_BY_PATH } = useQuery({
		...commentsQueryOptions(projectId),
		enabled: commentAnnotations,
		select: (comments) => annotationsByPathForScope(comments, fileParent),
	});
	// The query fallback covers loading; this also hides cached data after opting out.
	const annotationsByPath = commentAnnotations
		? loadedAnnotationsByPath
		: EMPTY_ANNOTATIONS_BY_PATH;

	// The forge numbers its diff comments against the branch head, so they
	// belong to the branch diff — `review-threads.ts` says why, and drops the
	// ones this view cannot place.
	const threadBranchName =
		fileParent._tag === "Branch"
			? branchDetailsParams(decodeBytes(fileParent.branchRef)).branchName
			: null;
	const { data: diffForgeInfo } = useQuery(forgeInfoOptions(projectId));
	const { data: threadReview } = useQuery({
		...listReviewsQueryOptions({ projectId, cacheConfig: "noCache" }),
		enabled: threadBranchName !== null && diffForgeInfo?.capabilities.prService === true,
		select: (reviews) => reviews.find((review) => review.sourceBranch === threadBranchName) ?? null,
	});
	const { data: threads } = useQuery({
		...listReviewThreadsQueryOptions({ projectId, reviewId: threadReview?.number ?? 0 }),
		enabled: threadReview != null,
	});
	// Grouped here rather than in `select`, which re-runs per render and would
	// hand the minimap a new map every time — its paint loop compares by
	// identity, so that would repaint the canvas on every scroll frame.
	const threadsByPath = useMemo(
		() =>
			threads === undefined ? EMPTY_THREADS_BY_PATH : threadsByPathForScope(threads, fileParent),
		[threads, fileParent],
	);

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

	// The forge's line number is only as good as the diff it was left on: an
	// amend or rebase it has not seen moves the code underneath. A thread
	// whose quoted line no longer matches is dropped rather than hung on
	// whatever now occupies that number — filtered once here, so every
	// surface reading the map (the annotations, their cards, the minimap's
	// pins) agrees on which threads exist.
	const anchoredThreadsByPath = useMemo((): ThreadsByPath => {
		if (threadsByPath.size === 0) return threadsByPath;
		const anchored = new Map<string, Array<AnchoredThread>>();
		for (const [path, threads] of threadsByPath) {
			const file = diffViewSansAnno.fileByPath.get(path);
			if (file === undefined) continue;
			const kept = threads.filter(({ thread, lineNumber, side }) =>
				threadStillAnchored(thread, lineNumber, side, file.item.fileDiff),
			);
			if (kept.length > 0) anchored.set(path, kept);
		}
		return anchored;
	}, [threadsByPath, diffViewSansAnno]);

	// Remapping every item hands identity-compared consumers a fresh tree;
	// memoized like its inputs so an unrelated render costs nothing.
	const diffView = useMemo(
		() => withAnnotations(diffViewSansAnno, annotationsByPath, anchoredThreadsByPath),
		[diffViewSansAnno, annotationsByPath, anchoredThreadsByPath],
	);
	const activeFileItemId =
		activeFilePath === null
			? null
			: (diffViewSansAnno.fileByPath.get(activeFilePath)?.item.id ?? null);
	const diffContextKey =
		shownFileIndex === null ? reviewedFilesContextId : (activeFileItemId ?? reviewedFilesContextId);

	const allFilesReviewed =
		preparedDiffFiles.length > 0 &&
		preparedDiffFiles.every(({ change, version }) => reviewedFiles.get(change.path)?.has(version));

	// Resolved once for the whole list rather than per row: a row would have to
	// find its own version to answer this.
	const reviewedFilePaths = reviewedPaths(preparedDiffFiles, reviewedFiles);

	const toggleAllFilesReviewed = (): void => {
		setManualCollapseByItem(new Map());
		setFilesReviewed({
			projectId,
			contextId: reviewedFilesContextId,
			files: preparedDiffFiles.map(({ change, version }) => ({ path: change.path, version })),
			reviewed: !allFilesReviewed,
		});
	};

	const activateRow = (selection: string) => {
		onPassiveFileSelection(selection);

		const path = selectedFilePath(filesRows, selection);
		const file = path === null ? undefined : diffViewSansAnno.fileByPath.get(path);
		if (!file) return;

		const firstHunk = file.hunks[0];
		onActiveFileSelection(
			file.item.id,
			firstHunk ? { file: file.address, range: firstHunk.selectedLines.range } : null,
		);
	};

	const filesPanelRef = useRef<HTMLDivElement>(null);
	const filesTreeRef = useRef<HTMLDivElement>(null);
	const fileFilter = useListFilter({
		filter: filesFilter,
		setFilter: (filter) => dispatch(projectSlice.actions.setFilesFilter({ projectId, filter })),
		inputId: "files-filter-input",
		subject: "files",
		scope: "files",
		selectionKey: filesSelection,
		firstKey: filesRows[0]?.path,
		onEnterList: () => {
			if (filesSelection !== null) activateRow(filesSelection);
		},
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

	// Hoisted out of the JSX below, where they used to be called inline: the
	// empty branch that follows returns before that JSX, and a hook reached only
	// on one branch is a hook called conditionally.
	const diffContentsRef = useMergedRefs(focusScopeRef, diffContentsEl, useAutofocusScope());

	// One statement, not three. With no changes the header badge already reads 0,
	// so a file list and a viewer both saying so as well would be the same fact
	// three times across two columns — and two empty columns read as broken
	// rather than as deliberate. The whole body becomes the one block instead.
	//
	// Conflicts are the exception, and not a cosmetic one: the conflict bar lives
	// in the layout below, and it carries the only route into edit mode from
	// here. A conflicted commit can hold no diffable changes at all, so
	// collapsing on the count alone takes that route away with them.
	if (changes.length === 0 && conflicts.length === 0 && manualConflicts.length === 0) {
		return (
			<div className={classes(styles.diffTab, styles.diffTabEmpty)}>
				<EmptyState
					illustration="waving"
					title="No file changes"
					description={
						fileParent._tag === "Commit"
							? "This commit changes no files"
							: "Nothing on this branch changes any files"
					}
				/>
			</div>
		);
	}

	return (
		<div className={styles.diffTab}>
			<Group
				id={layoutId}
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
									<ListFilterRow {...fileFilter.rowProps} />
								)}
								<div
									className={classes(
										uiStyles.scroller,
										uiStyles.scrollerWithSeparator,
										styles.diffFiles,
									)}
								>
									<FilesTree
										focusScope="files"
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
										addressSpace={filesAddressSpace}
										fileParent={fileParent}
										reviewedPaths={reviewedFilePaths}
										canUncommit={!isCommitUncommitChangesPending}
										uncommit={uncommit}
										emptyLabel={
											filesFilter !== null && filesItems.length > 0
												? "No matching files."
												: undefined
										}
										ref={filesTreeRef}
									/>
								</div>
							</div>
						</Panel>
						<ResizeHandle />
					</>
				)}

				<Panel id={"diff-panel" satisfies PanelId} minSize={300} className={styles.panel}>
					<div className={styles.actions}>
						{canShowFiles && <FilesToggle projectId={projectId} />}

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
								{allFilesReviewed ? "Mark all unviewed" : "Mark all viewed"}
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
							data-focus-scope={"diff" satisfies FocusScope}
							// oxlint-disable-next-line jsx_a11y/no-noninteractive-tabindex -- Revisit this when we add hunk/line selection.
							tabIndex={0}
							className={styles.diffContentsContainer}
							ref={diffContentsRef}
						>
							<DiffContents
								activeFileItemId={activeFileItemId}
								diffContextKey={diffContextKey}
								onViewerFileSelection={onPassiveFileSelection}
								fileParent={fileParent}
								projectId={projectId}
								diffView={diffView}
								annotationsByPath={annotationsByPath}
								threadsByPath={anchoredThreadsByPath}
								threadReviewId={threadReview?.number ?? 0}
								diffBackgrounds={diffSettings?.diffBackground}
								diffOverflow={diffSettings?.diffOverflow}
								diffStyle={diffStyle}
								commentAnnotations={commentAnnotations}
								reviewedFiles={reviewedFiles}
								manualCollapseByItem={manualCollapseByItem}
								setManualCollapse={setManualCollapse}
								setFilesReviewed={setFilesReviewed}
								canUncommit={!isCommitUncommitChangesPending}
								uncommit={uncommit}
								focusScopeRef={focusScopeRef}
								viewerRef={viewerRef}
								didScrollToViaFileRef={didScrollToViaFileRef}
								minimapFiles={minimapShown ? minimapFiles : null}
							/>
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
						<Icon name="commit" />
						<h3 className={classes("text-15", "text-semibold")}>Loading…</h3>
					</div>
				</div>
			</div>
		</div>
	);
};

const CommitDetails: FC<{
	selection: Extract<Address, { _tag: "Commit" }>;
	projectId: string;
	/** The merged review the commit landed, when known: adds a Pull Request tab. */
	review?: TargetCommitReview | null;
	onActiveFileSelection: (itemId: string, firstSelection: DiffLineSelection | null) => void;
	viewerRef: RefObject<DiffViewerHandle | null>;
	didScrollToViaFileRef: RefObject<boolean>;
}> = ({
	selection,
	review,
	projectId,
	onActiveFileSelection,
	viewerRef,
	didScrollToViaFileRef,
}) => {
	const detailsFullWindow = useAppSelector(interfaceSlice.selectors.selectDetailsFullWindow);
	const filesVisibleState = useAppSelector((state) =>
		projectSlice.selectors.selectFilesVisible(state, projectId),
	);
	const canShowFiles = useCanShowFiles();
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
		setCursor("files", selection);
	};

	// The tab is per selection: selecting another commit remounts this
	// component, and a landed review is read, not worked on, so nothing needs
	// to remember the choice. The review is what the commit is listed for, so
	// it opens on the review — derived rather than seeded into the state, as
	// the review can resolve after mount.
	const [chosenTab, setTab] = useState<BranchTab | null>(null);
	const tab = chosenTab ?? (review ? "pr" : "diff");
	const ref = useRef<HTMLDivElement>(null);
	useBranchTabHotkeys({ branchTab: tab, setBranchTab: setTab, target: ref, enabled: !!review });

	return (
		<div className={styles.container} ref={ref}>
			<div className={styles.headerWrap}>
				<div className={styles.titleRow}>
					{detailsFullWindow && <TopLeftControls />}

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
					{review && (
						<BranchTabToggle
							branchTab={tab}
							setBranchTab={setTab}
							className={styles.commitDetailsMetaTabs}
						/>
					)}
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

			{review && tab === "pr" ? (
				<div className={styles.prTabScroll}>
					<div className={styles.prTab}>
						<LandedReviewView projectId={projectId} reviewId={review.number} />
					</div>
				</div>
			) : (
				<Diff
					changes={changes}
					filesVisible={filesVisible}
					canShowFiles={canShowFiles}
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
			)}
		</div>
	);
};

/**
 * A landed review fetched by number, for the surfaces that only know the
 * number: the target-commit listing, the branch listing's merged review, and
 * an integrated applied branch's stored identity.
 */
const LandedReviewView: FC<{ projectId: string; reviewId: number }> = ({ projectId, reviewId }) => {
	const { data: review, isError } = useQuery(getReviewQueryOptions({ projectId, reviewId }));
	if (isError) {
		return (
			<div className={classes(styles.loadingTab, "text-13")}>Could not load the pull request.</div>
		);
	}
	if (!review) return <div className={classes(styles.loadingTab, "text-13")}>Loading…</div>;
	// Keyed: a switch to another review is a new visit, so the arrival
	// snapshot and its markers start clean rather than surviving in place.
	return (
		<ReviewView
			key={review.number}
			projectId={projectId}
			sourceBranch={review.sourceBranch}
			review={review}
		/>
	);
};

/** A branch's own changes, whatever the branch's standing. */
const BranchDiff: FC<BranchDetailsProps> = ({
	branch,
	projectId,
	onActiveFileSelection,
	viewerRef,
	didScrollToViaFileRef,
}) => {
	const filesVisibleState = useAppSelector((state) =>
		projectSlice.selectors.selectFilesVisible(state, projectId),
	);
	const canShowFiles = useCanShowFiles();
	const filesVisible = canShowFiles && filesVisibleState;

	const selectFile = (selection: string) => {
		setCursor("files", selection);
	};

	return (
		<SuspenseQuery
			{...branchDiffQueryOptions({ projectId, branch: decodeBytes(branch.branchRef) })}
		>
			{({ data: branchDiff }) => (
				<Diff
					changes={branchDiff.changes}
					filesVisible={filesVisible}
					canShowFiles={canShowFiles}
					filesItems={branchDiff.changes.map((change) =>
						changeFileRowItem({
							change,
							path: change.path,
							dependencyCommitIds: [],
						}),
					)}
					onPassiveFileSelection={selectFile}
					selection={branchAddress(branch)}
					projectId={projectId}
					onActiveFileSelection={onActiveFileSelection}
					viewerRef={viewerRef}
					didScrollToViaFileRef={didScrollToViaFileRef}
				/>
			)}
		</SuspenseQuery>
	);
};

const BranchTitleRow: FC<{ branchName: string }> = ({ branchName }) => {
	const detailsFullWindow = useAppSelector(interfaceSlice.selectors.selectDetailsFullWindow);

	return (
		<div className={styles.titleRow}>
			{detailsFullWindow && <TopLeftControls />}

			<div className={styles.title}>
				<Icon name="branch" />
				<h3 className={classes(styles.titleContent, "text-15", "text-semibold")}>{branchName}</h3>
			</div>
		</div>
	);
};

/**
 * The Diff / Pull Request toggle. A branch with no review keeps the toggle —
 * the tab goes disabled and says so, where dropping the toggle would instead
 * read as the control having gone missing. The reason rides in the label
 * because a disabled button takes no pointer events, so a tooltip on it would
 * never open.
 */
const BranchTabToggle: FC<{
	branchTab: BranchTab;
	setBranchTab: (tab: BranchTab) => void;
	prDisabled?: boolean;
	/** Marks the Pull Request tab with an unread-activity dot. */
	prUnread?: boolean;
	className?: string;
}> = ({ branchTab, setBranchTab, prDisabled = false, prUnread = false, className }) => (
	<ToggleGroup
		render={<ToggleGroupStyles className={className} />}
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
		<Toggle render={<ToggleStyles />} value={"pr" satisfies BranchTab} disabled={prDisabled}>
			{prDisabled ? "No pull request" : "Pull Request"}
			{!prDisabled && prUnread && (
				<span className={rowStyles.unreadDot}>
					<span className={rowStyles.unreadLabel}>New activity</span>
				</span>
			)}
		</Toggle>
	</ToggleGroup>
);

/** `[` and `]` step between a branch's tabs; with two of them, either key toggles. */
const useBranchTabHotkeys = ({
	branchTab,
	setBranchTab,
	target,
	enabled = true,
}: {
	branchTab: BranchTab;
	setBranchTab: (tab: BranchTab) => void;
	target: RefObject<HTMLElement | null>;
	enabled?: boolean;
}) => {
	const toggle = () => {
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
	};

	useHotkeys(
		(["[", "]"] as const).map((hotkey) => ({
			hotkey,
			callback: toggle,
			options: { conflictBehavior: "allow" as const, enabled, target },
		})),
	);
};

/**
 * An existing review: its description, the conversation and the side panel.
 * Editing is the applied branch's affordance — the branches tab shows a review,
 * it does not work on one.
 */
const ReviewLayout: FC<{
	projectId: string;
	sourceBranch: string;
	review: ForgeReview;
	editing?: { active: boolean; onDone: () => void };
}> = ({ projectId, sourceBranch, review, editing }) => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	// The level is read unconditionally: behind `&&` the hook would be skipped
	// on the render before the forge answers, changing the hook count.
	const notificationsLevel = usePrNotificationsLevel();
	useMarkReviewSeenOnView(
		projectId,
		review,
		forgeInfo?.capabilities.prService === true && notificationsLevel !== "off",
	);
	const hasConversation = forgeInfo?.capabilities.reviewComments !== false;

	return (
		<div className={styles.prLayout}>
			<div className={styles.prMain}>
				<PullRequestDescription
					key={review.number}
					body={review.body}
					projectId={projectId}
					reviewId={review.number}
					sourceBranch={sourceBranch}
					title={review.title}
					canSubmit={editing !== undefined}
					editing={editing?.active ?? false}
					onDoneEditing={() => editing?.onDone()}
				/>

				{hasConversation && <PullRequestComments projectId={projectId} review={review} />}
			</div>

			<PullRequestPanel
				projectId={projectId}
				review={review}
				activity={
					hasConversation ? <ReviewTimeline projectId={projectId} review={review} /> : undefined
				}
			/>
		</div>
	);
};

/**
 * An existing review, with what had been seen at arrival snapshotted so the
 * conversation can badge what is actually new.
 */
const ReviewView: FC<ComponentProps<typeof ReviewLayout>> = (p) => {
	const seenOnArrival = useSeenOnArrival(p.projectId, p.review.number);
	return (
		<SeenOnArrivalContext.Provider value={seenOnArrival}>
			<ReviewLayout {...p} />
		</SeenOnArrivalContext.Provider>
	);
};

/**
 * The Pull Request tab for a branch that has no PR yet: the create form, with
 * the panel beside it summarizing what would be published.
 */
const NewPullRequestView: FC<{
	projectId: string;
	branchName: string;
	targetBranch: string | undefined;
	canSubmit: boolean;
}> = ({ projectId, branchName, targetBranch, canSubmit }) => {
	// Same record the form persists its title and body to, read here for the
	// fields the panel owns. Both writers merge, so neither wipes the other.
	const { data: draft } = useSuspenseQuery(draftPRQueryOptions({ projectId, branchName }));
	const { mutate: persistDraftPR } = usePersistDraftPR();
	const [extras, setExtras] = useState<DraftPRExtras>({
		labels: draft?.labels ?? [],
		reviewers: draft?.reviewers ?? [],
	});

	const changeExtras = (next: DraftPRExtras) => {
		setExtras(next);
		persistDraftPR({ projectId, branchName, draft: { ...draft, ...next } });
	};

	const { mutate: addReviewLabels } = useAddReviewLabels(projectId);
	const { mutate: requestReview } = useRequestReview(projectId);

	// The forge takes none of these when a PR is created — GitHub's create
	// endpoint accepts neither labels nor reviewers — so they are applied the
	// moment the PR exists. Each mutation toasts its own failure and the PR
	// stands regardless; the real panel replaces this one and can set by hand
	// whatever did not land.
	const applyExtras = (reviewId: number) => {
		if (extras.labels.length > 0) addReviewLabels({ projectId, reviewId, labels: extras.labels });
		if (extras.reviewers.length > 0)
			requestReview({ projectId, reviewId, logins: extras.reviewers });
	};

	return (
		<div className={styles.prLayout}>
			<div className={styles.prMain}>
				<PullRequestForm
					key={branchName}
					body={null}
					projectId={projectId}
					reviewId={null}
					sourceBranch={branchName}
					title={null}
					canSubmit={canSubmit}
					afterPublish={applyExtras}
				/>
			</div>

			<NewPullRequestPanel
				projectId={projectId}
				sourceBranch={branchName}
				targetBranch={targetBranch}
				extras={extras}
				onExtrasChange={changeExtras}
			/>
		</div>
	);
};

/** What every details view threads through to its Diff. */
type DetailsViewProps = {
	projectId: string;
	onActiveFileSelection: (itemId: string, firstSelection: DiffLineSelection | null) => void;
	viewerRef: RefObject<DiffViewerHandle | null>;
	didScrollToViaFileRef: RefObject<boolean>;
};

type BranchDetailsProps = { branch: BranchAddress } & DetailsViewProps;

/**
 * A branch the workspace does not hold, as the branches tab lists them: its
 * changes, and its review when one already exists. Opening a review is not
 * offered — the base comes from a branch's position in a workspace stack, which
 * this branch has not got, so `publish_review` refuses it.
 */
const UnappliedBranchDetails: FC<BranchDetailsProps> = ({
	branch,
	projectId,
	onActiveFileSelection,
	viewerRef,
	didScrollToViaFileRef,
}) => {
	const dispatch = useAppDispatch();
	const branchName = branchDetailsParams(decodeBytes(branch.branchRef)).branchName;
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	// Same query key as the applied branch's, so the two share one listing
	// rather than polling the forge twice. Reviews are keyed by branch name,
	// which says nothing about whether the branch is applied.
	const { data: review } = useQuery({
		...listReviewsQueryOptions({ projectId, cacheConfig: "noCache" }),
		enabled: forgeInfo?.capabilities.prService === true,
		select: (reviews) => reviews.find((review) => review.sourceBranch === branchName) ?? null,
	});
	// A merged review has left the open listing; the branch listing's cached
	// review — the same source this branch's row chip renders, with the
	// merged/closed distinction derived server-side — still knows its number.
	// Unapplied branches have no durable stored identity (their metadata is
	// garbage-collected once integrated), so the cache is the source here.
	const { data: listedLandedNumber } = useQuery({
		...branchListQueryOptions(projectId),
		select: (stacks) =>
			stacks
				.values()
				.flatMap((stack) => stack.branches)
				.find((listed) => listed.displayName === branchName && listed.reviewStatus === "merged")
				?.review?.number ?? null,
	});
	const landedReviewId = listedLandedNumber ?? null;

	const reviewTab = review ? (
		<ReviewView
			key={review.number}
			projectId={projectId}
			sourceBranch={branchName}
			review={review}
		/>
	) : landedReviewId !== null ? (
		<LandedReviewView projectId={projectId} reviewId={landedReviewId} />
	) : null;

	const notificationsLevel = usePrNotificationsLevel();
	const prUnread = useReviewUnread(
		projectId,
		{ number: review?.number ?? 0, modifiedAt: review?.modifiedAt ?? null },
		review != null && forgeInfo?.capabilities.prService === true && notificationsLevel !== "off",
	);

	const chosenTab = useAppSelector((state) =>
		projectSlice.selectors.selectBranchTab(state, projectId, branchName),
	);
	// The review is what the branch is judged by, so a branch that has one —
	// open or landed — opens on it; without one only the diff is on offer.
	const branchTab = chosenTab ?? (reviewTab !== null ? "pr" : "diff");
	const setBranchTab = (tab: BranchTab) => {
		dispatch(projectSlice.actions.setSelectedBranchTab({ projectId, branchName, tab }));
	};

	const ref = useRef<HTMLDivElement>(null);
	// The review is the only second tab on offer here, so the toggle and the keys
	// that drive it both wait for one to exist.
	useBranchTabHotkeys({ branchTab, setBranchTab, target: ref, enabled: reviewTab !== null });

	const { isPending: isApplyPending, apply } = useApplyToWorkspace(projectId);

	return (
		<div className={styles.container} ref={ref}>
			<div className={styles.headerWrap}>
				<BranchTitleRow branchName={branchName} />

				<div className={styles.tabsRow}>
					<BranchTabToggle
						branchTab={branchTab}
						setBranchTab={setBranchTab}
						prDisabled={reviewTab === null}
						prUnread={prUnread}
					/>

					<div className={styles.tabsRowRight}>
						<button
							type="button"
							className={getButtonClassName({ variant: "gray" })}
							disabled={isApplyPending}
							onClick={() => apply(decodeBytes(branch.branchRef))}
						>
							{isApplyPending && <Icon name="spinner" />}
							Apply to workspace
						</button>
					</div>
				</div>
			</div>

			<Suspense fallback={<div className={classes(styles.loadingTab, "text-13")}>Loading…</div>}>
				{reviewTab !== null && branchTab === "pr" ? (
					<div className={styles.prTabScroll}>
						<div className={styles.prTab}>{reviewTab}</div>
					</div>
				) : (
					<BranchDiff
						projectId={projectId}
						branch={branch}
						onActiveFileSelection={onActiveFileSelection}
						viewerRef={viewerRef}
						didScrollToViaFileRef={didScrollToViaFileRef}
					/>
				)}
			</Suspense>
		</div>
	);
};

/** A branch applied to the workspace: its changes, and the review of them. */
const AppliedBranchDetails: FC<BranchDetailsProps> = ({
	branch,
	projectId,
	onActiveFileSelection,
	viewerRef,
	didScrollToViaFileRef,
}) => {
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const headInfoIndex = headInfo ? getHeadInfoIndex(headInfo) : null;
	const dispatch = useAppDispatch();
	const branchRef = decodeBytes(branch.branchRef);
	const branchName = branchDetailsParams(branchRef).branchName;
	const chosenTab = useAppSelector((state) =>
		projectSlice.selectors.selectBranchTab(state, projectId, branchName),
	);
	// The review is where an applied branch is headed, so a forge that serves
	// pull requests opens on that tab — the create form when none exists yet.
	// Without such a forge the tab is a dead form, so the diff leads.
	const branchTab = chosenTab ?? (forgeInfo?.capabilities.prService ? "pr" : "diff");

	const setBranchTab = (tab: BranchTab) => {
		dispatch(projectSlice.actions.setSelectedBranchTab({ projectId, branchName, tab }));
	};

	// Per-PR by construction: BranchDetails is keyed on the branch identity,
	// so a selection change remounts this component and resets the mode.
	const [prEditing, setPrEditing] = useState(false);

	const startPrEdit = () => {
		setBranchTab("pr");
		setPrEditing(true);
	};

	const ref = useRef<HTMLDivElement>(null);
	useBranchTabHotkeys({ branchTab, setBranchTab, target: ref });

	// Use push status of segment, not branch details; something about remote
	// tracking refs.
	const branchCtx = headInfoIndex?.branchContextByRefBytes(branch.branchRef);
	const parentSegment = branchCtx?.stack.segments[branchCtx.segmentIndex + 1];
	const targetBranch =
		!parentSegment || parentSegment.pushStatus === "integrated"
			? headInfo?.target?.remoteTrackingRef.displayName
			: parentSegment.pushStatus === "completelyUnpushed"
				? undefined
				: parentSegment.refName?.displayName;

	// The open listing already carries everything an open review needs, so the
	// verification fetch is spent only when the listing has nothing for this
	// branch — the case where the recorded number's fate actually decides the
	// tab between the landed review and the create-PR flow.
	const { data: hasOpenReview } = useQuery({
		...listReviewsQueryOptions({ projectId, cacheConfig: "noCache" }),
		// The listing is alive anyway (every branch row subscribes to it), so
		// this gate expresses intent rather than saving a fetch.
		enabled: branchTab === "pr" && !!forgeInfo?.capabilities.prService,
		select: (reviews) => reviews.some((review) => review.sourceBranch === branchName),
	});
	const landedReviewId = useLandedReviewId(
		projectId,
		branchCtx ? recordedPullRequest(branchCtx.segment) : null,
		branchTab === "pr" && hasOpenReview === false,
	);

	// Subscribed regardless of the chosen tab: the dot on the toggle is what
	// tells a reader parked on the diff that the review moved.
	const notificationsLevel = usePrNotificationsLevel();
	const { data: openReview } = useQuery({
		...listReviewsQueryOptions({ projectId, cacheConfig: "noCache" }),
		enabled: !!forgeInfo?.capabilities.prService && notificationsLevel !== "off",
		select: (reviews) => reviews.find((review) => review.sourceBranch === branchName) ?? null,
	});
	const prUnread = useReviewUnread(
		projectId,
		{ number: openReview?.number ?? 0, modifiedAt: openReview?.modifiedAt ?? null },
		!!openReview && !!forgeInfo?.capabilities.prService && notificationsLevel !== "off",
	);

	return (
		<div className={styles.container} ref={ref}>
			<div className={styles.headerWrap}>
				<BranchTitleRow branchName={branchName} />

				<div className={styles.tabsRow}>
					<BranchTabToggle branchTab={branchTab} setBranchTab={setBranchTab} prUnread={prUnread} />

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
												review={review}
												isEditing={prEditing}
												onStartEdit={startPrEdit}
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
					<div className={styles.prTabScroll}>
						<div className={styles.prTab}>
							{!forgeInfo?.capabilities.prService ? (
								<NewPullRequestView
									projectId={projectId}
									branchName={branchName}
									targetBranch={targetBranch}
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
										const canSubmit =
											targetBranch !== undefined &&
											branchCtx?.segment.pushStatus !== "completelyUnpushed";

										if (!review && landedReviewId !== null)
											return <LandedReviewView projectId={projectId} reviewId={landedReviewId} />;

										return !review || !canSubmit ? (
											<NewPullRequestView
												projectId={projectId}
												branchName={branchName}
												targetBranch={targetBranch}
												canSubmit={canSubmit}
											/>
										) : (
											<ReviewView
												key={review.number}
												projectId={projectId}
												sourceBranch={branchName}
												review={review}
												editing={{ active: prEditing, onDone: () => setPrEditing(false) }}
											/>
										);
									}}
								</SuspenseQuery>
							)}
						</div>
					</div>
				) : (
					<BranchDiff
						projectId={projectId}
						branch={branch}
						onActiveFileSelection={onActiveFileSelection}
						viewerRef={viewerRef}
						didScrollToViaFileRef={didScrollToViaFileRef}
					/>
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
	path: string;
	projectId: string;
	onActiveFileSelection: (itemId: string, firstSelection: DiffLineSelection | null) => void;
	viewerRef: RefObject<DiffViewerHandle | null>;
	didScrollToViaFileRef: RefObject<boolean>;
}> = ({ path, projectId, onActiveFileSelection, viewerRef, didScrollToViaFileRef }) => {
	const detailsFullWindow = useAppSelector(interfaceSlice.selectors.selectDetailsFullWindow);
	// This view is the uncommitted scope, and the sidebar's own "Uncommitted"
	// list is already its files panel — a second one here would only repeat it,
	// so the user's files-visible setting has nothing to apply to.
	//
	// A constant rather than `useCanShowFiles()`, which reads the URL's active
	// list as a proxy for what drives the pane: the pane can be driven by the
	// uncommitted list while the param still names the applied one, and the
	// proxy then answers for the wrong scope.
	const canShowFiles = false;
	const filesVisible = false;
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));
	const filesItems = getChangesFileRowItems(worktreeChanges).toArray();
	const changes = filesItems
		.values()
		.map((item) => (item._tag === "Change" ? item.change : null))
		.filter((x) => x != null)
		.toArray();

	const selectFile = (selection: string) => {
		setCursor("uncommitted", selection);
	};

	const title = (
		<>
			{detailsFullWindow && <TopLeftControls />}

			<div className={styles.title}>
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
					canShowFiles={canShowFiles}
					filesItems={filesItems}
					onPassiveFileSelection={selectFile}
					selection={fileAddress({ parent: uncommittedChangesFileParent, path })}
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

/** A commit selection is shown the same way whichever page selected it. */
const commitDetails = (
	commit: Extract<Address, { _tag: "Commit" }>,
	viewProps: DetailsViewProps,
	review?: TargetCommitReview | null,
): ReactNode => (
	<Suspense fallback={<CommitDetailsSkeleton />}>
		<CommitDetails
			key={weakCommitIdentityKey(commit)}
			selection={commit}
			review={review}
			{...viewProps}
		/>
	</Suspense>
);

/**
 * The one details pane for every address-carrying selection. It dispatches on
 * the selection itself: a branch is applied or unapplied by what the
 * workspace holds (`headInfo`), not by which page selected it — the address
 * says what the value is; the data says how it stands. A commit is shown the
 * same way whichever page selected it.
 */
export const Details: FC<
	{ selection: Address | null; review: TargetCommitReview | null } & DetailsViewProps
> = ({ selection, review, ...viewProps }) => {
	const { projectId } = viewProps;
	// The Pull Request tab fetches the review from the forge, so it is only
	// offered on a forge that serves them.
	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const landedReview = forgeInfo?.capabilities.prService ? review : null;

	// No selection exists before the lists render, and the lists derive from
	// headInfo — by the time anything is selectable, it has loaded.
	if (!selection || !headInfo) return null;

	return Match.value(selection).pipe(
		Match.tags({
			Branch: (branch) =>
				getHeadInfoIndex(headInfo).isApplied(branch.branchRef) ? (
					<AppliedBranchDetails key={branchIdentityKey(branch)} branch={branch} {...viewProps} />
				) : (
					<UnappliedBranchDetails key={branchIdentityKey(branch)} branch={branch} {...viewProps} />
				),
			Commit: (commit) => commitDetails(commit, viewProps, landedReview),
		}),
		Match.orElse(() => null),
	);
};

/** The details pane for the uncommitted-files scope. */
export const UncommittedFilesDetails: FC<{ path: string } & DetailsViewProps> = (p) => (
	<Suspense fallback={<FileDetailsSkeleton />}>
		<FileDetails {...p} />
	</Suspense>
);
