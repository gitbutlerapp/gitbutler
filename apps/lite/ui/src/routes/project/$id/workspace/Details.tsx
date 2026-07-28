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
	weakFileIdentityKey,
	weakFileParentIdentityKey,
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
	type GetHoveredLineResult,
	type DiffLineAnnotation,
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
import { Annotation } from "#ui/components/Annotation.tsx";
import {
	createLocalAnnotation,
	feedbackPrompt,
	localAnnotationsQueryOptions,
	type LocalAnnotation as PersistedLocalAnnotation,
	type LocalAnnotationsByPath,
	usePersistLocalAnnotations,
} from "#ui/annotation.ts";

type Annotation = { _tag: "local"; id: string };

type BranchTab = "diff" | "pr";

// This must be unique as to not collide with other IDs, and stable because it's
// stored in local storage.
type PanelId = "files-panel" | "diff-panel";

const diffDefaults = {
	diffBackground: true,
	diffOverflow: "scroll",
	diffStyle: "split",
} satisfies Partial<GUISettings>;

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
	id: string,
	change: TreeChange,
	hunks: Array<DiffHunk>,
	annotations: Array<PersistedLocalAnnotation>,
): CodeViewDiffItem<Annotation> => {
	const combinedFilePatch = synthesizeFilePatch(change, hunks);
	const version = hash(combinedFilePatch);
	const parsed = parsePatchFiles(combinedFilePatch, String(version));

	const [patch, ...restPatches] = parsed;
	if (!patch) throw new Error("Failed to parse any patches");
	if (restPatches.length > 0) throw new Error("Parsed more than one patch");

	const [fileDiff, ...restFiles] = patch.files;
	if (!fileDiff) throw new Error("Failed to parse any files in patch");
	if (restFiles.length > 0) throw new Error("Parsed more than one file in patch");

	const itemAnnotations: Array<DiffLineAnnotation<Annotation>> = annotations.map(
		({ id, lineNumber, side }) => ({
			lineNumber,
			side,
			metadata: { _tag: "local", id },
		}),
	);

	return {
		type: "diff",
		id,
		version: combineHashes(version, itemAnnotations.length),
		fileDiff,
		annotations: itemAnnotations,
	};
};

type DiffViewDeps = {
	fileParent: FileParent;
	changes: Array<TreeChange>;
	treeChangeDiffs: Array<UnifiedPatch | null>;
	annotationsByPath: LocalAnnotationsByPath;
};

type DiffViewFile = {
	operand: FileOperand;
	item: CodeViewDiffItem<Annotation>;
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
	items: Array<CodeViewDiffItem<Annotation>>;
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
	annotationsByPath,
}: DiffViewDeps): DiffView => {
	const navigationIndex: NavigationIndex<HunkOperand> = {
		items: [],
		indexByKey: new Map(),
	};

	const items: Array<CodeViewDiffItem<Annotation>> = [];

	const fileByItemId = new Map<string, DiffViewFile>();
	const fileByPath = new Map<string, DiffViewFile>();
	const fileByHunkKey = new Map<string, DiffViewFile>();
	const hunkByKey = new Map<string, DiffViewHunk>();

	for (const [ci, change] of changes.entries()) {
		const mdiff = treeChangeDiffs[ci];
		const annotations = annotationsByPath.get(change.path) ?? [];

		const file: FileOperand = {
			parent: fileParent,
			path: change.path,
		};
		const item = mkCodeViewItem(
			weakFileIdentityKey(file),
			change,
			mdiff && "subject" in mdiff && "hunks" in mdiff.subject ? mdiff.subject.hunks : [],
			annotations,
		);

		items.push(item);

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
	localAnnotationFormId: string;
	selectionScopeRef: RefObject<HTMLDivElement | null>;
	onViewerFileSelection: (path: string) => void;
	fileParent: FileParent;
	projectId: string;
	diffView: DiffView;
	annotationsByPath: LocalAnnotationsByPath;
	diffBackgrounds?: GUISettings["diffBackground"];
	diffOverflow?: GUISettings["diffOverflow"];
	diffStyle?: GUISettings["diffStyle"];
	viewerRef: RefObject<CodeViewHandle<Annotation> | null>;
	didScrollToViaFileRef: RefObject<boolean>;
}> = ({
	localAnnotationFormId,
	selectionScopeRef,
	onViewerFileSelection,
	fileParent,
	projectId,
	diffView: { items, navigationIndex, hunkByKey, fileByHunkKey, fileByItemId },
	annotationsByPath,
	diffBackgrounds,
	diffOverflow,
	diffStyle,
	viewerRef,
	didScrollToViaFileRef,
}) => {
	const [collapsedItems, setCollapsedItems] = useState<Set<string>>(new Set());
	const newFocusableAnnotationIdRef = useRef<string | null>(null);
	const dispatch = useAppDispatch();
	const { mutate: persistLocalAnnotations } = usePersistLocalAnnotations();

	const persistFileAnnotations = (
		path: string,
		fileAnnotations: Array<PersistedLocalAnnotation>,
	) => {
		const updated = new Map(annotationsByPath);
		if (fileAnnotations.length === 0) updated.delete(path);
		else updated.set(path, fileAnnotations);

		persistLocalAnnotations({
			projectId,
			fileParentKey: weakFileParentIdentityKey(fileParent),
			annotations: updated,
		});
	};
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

	const handleHunkPostRender = useDiffHunkDrag<Annotation>({
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
	const enhanceCollapsed = <T,>(item: CodeViewDiffItem<T>): CodeViewDiffItem<T> => ({
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
			renderGutterUtility={(getHoveredLine, item) => {
				const handleClick = () => {
					const badlyTypedLine = getHoveredLine();
					if (!badlyTypedLine || !("side" in badlyTypedLine)) return;
					const line = badlyTypedLine as GetHoveredLineResult<"diff">;

					const file = fileByItemId.get(item.id);
					if (!file) return;

					const annotation = createLocalAnnotation(line.lineNumber, line.side);
					newFocusableAnnotationIdRef.current = annotation.id;
					persistFileAnnotations(file.operand.path, [
						...(annotationsByPath.get(file.operand.path) ?? []),
						annotation,
					]);
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
				if (!("side" in anno)) throw new Error("Only diff items may be rendered");

				const file = fileByItemId.get(item.id);
				if (!file) return null;

				const annotations = annotationsByPath.get(file.operand.path) ?? [];
				const idx = annotations.findIndex(({ id }) => id === anno.metadata.id);
				const annotation = annotations[idx];
				if (!annotation) return null;

				return (
					<Annotation
						author="You"
						defaultBody={annotation.body}
						formId={localAnnotationFormId}
						name={annotation.id}
						textareaRef={(textarea) => {
							if (textarea && newFocusableAnnotationIdRef.current === annotation.id) {
								newFocusableAnnotationIdRef.current = null;
								textarea.focus();
							}
						}}
						// Pierre gracefully blurs focused annotations before virtualizing their item,
						// permitting uncontrolled input state to be persisted.
						onBlur={(body) =>
							persistFileAnnotations(
								file.operand.path,
								annotations.with(idx, { ...annotation, body }),
							)
						}
						actions={
							<>
								<button
									type="button"
									className={getButtonClassName({ variant: "ghost" })}
									onClick={() => {
										persistFileAnnotations(file.operand.path, annotations.toSpliced(idx, 1));
										selectionScopeRef.current?.focus({ focusVisible: false });
									}}
								>
									Delete
								</button>

								<button
									type="button"
									form={localAnnotationFormId}
									className={getButtonClassName({ variant: "ghost" })}
									onClick={(evt) => {
										const form = evt.currentTarget.form;
										if (!form) throw new Error("Missing owning form");

										const body = new FormData(form).get(annotation.id);
										if (typeof body !== "string") throw new Error("Missing or invalid body");

										void window.lite.clipboardWriteText(
											feedbackPrompt([
												{
													annotation: { ...annotation, body },
													fileParent,
													path: file.operand.path,
												},
											]),
										);
									}}
								>
									Copy as prompt
								</button>

								<button
									type="button"
									form={localAnnotationFormId}
									className={getButtonClassName({ variant: "ghost" })}
									onClick={(evt) => {
										const form = evt.currentTarget.form;
										if (!form) throw new Error("Missing owning form");

										const formData = new FormData(form);
										const feedback = annotationsByPath
											.entries()
											.flatMap(([path, annotations]) =>
												annotations.map((annotation) => {
													// Use any live mounted bodies, falling back to persisted.
													const formBody = formData.get(annotation.id);
													return {
														annotation: {
															...annotation,
															body: typeof formBody === "string" ? formBody : annotation.body,
														},
														fileParent,
														path,
													};
												}),
											)
											.toArray();

										void window.lite.clipboardWriteText(feedbackPrompt(feedback));
									}}
								>
									Copy all as prompt
								</button>
							</>
						}
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
	item: CodeViewDiffItem<unknown>;
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
	selection: Operand;
	projectId: string;
}> = ({ changes, filesVisible, filesItems, onFileSelection, selection, projectId }) => {
	const localAnnotationFormId = useId();
	const selectionScopeRef = useRef<HTMLDivElement>(null);
	const viewerRef = useRef<CodeViewHandle<Annotation>>(null);

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

	const treeChangeDiffs = useSuspenseQueries({
		queries: changes.map((change) => treeChangeDiffsQueryOptions({ projectId, change })),
		combine: (results) => results.map((result) => result.data),
	});
	const { data: annotationsByPath } = useSuspenseQuery(
		localAnnotationsQueryOptions({
			projectId,
			fileParentKey: weakFileParentIdentityKey(fileParent),
		}),
	);

	const diffView = getDiffView({
		fileParent,
		changes,
		treeChangeDiffs,
		annotationsByPath,
	});

	const selectFileAndNavigateDiff = (selection: string) => {
		onFileSelection(selection);

		const file = diffView.fileByPath.get(selection);
		if (!file) return;

		dispatch(
			projectSlice.actions.selectDiff({
				projectId,
				selection: file.hunks[0]?.operand ?? null,
			}),
		);

		didScrollToViaFileRef.current = true;
		viewerRef.current?.scrollTo({
			type: "item",
			id: file.item.id,
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
			<form id={localAnnotationFormId} hidden />

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
							localAnnotationFormId={localAnnotationFormId}
							onViewerFileSelection={onFileSelection}
							fileParent={fileParent}
							projectId={projectId}
							diffView={diffView}
							annotationsByPath={annotationsByPath}
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

const CommitDetails: FC<{
	selection: Extract<Operand, { _tag: "Commit" }>;
}> = ({ selection }) => {
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

	const selectFile = (selection: string) => {
		dispatch(projectSlice.actions.selectFiles({ projectId, selection }));
	};

	return (
		<div className={styles.container}>
			<div className={styles.headerWrap}>
				<div className={styles.titleRow}>
					{detailsFullWindow && <TopLeftControls />}

					<SuspenseQuery
						{...commitDetailsWithLineStatsQueryOptions({
							projectId,
							commitId: selection.commitId,
						})}
					>
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
												aria-controls={commitBodyId}
												aria-expanded={!commitBodyCollapsed}
												aria-label={
													commitBodyCollapsed ? "Expand commit body" : "Collapse commit body"
												}
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
						)}
					</SuspenseQuery>
				</div>

				<CommitDetailsContent
					bodyCollapsed={commitBodyCollapsed}
					bodyId={commitBodyId}
					projectId={projectId}
					commitId={selection.commitId}
				/>
			</div>

			<Suspense fallback={<div className={classes(styles.loadingTab, "text-13")}>Loading…</div>}>
				<SuspenseQuery
					{...commitDetailsWithLineStatsQueryOptions({
						projectId,
						commitId: selection.commitId,
					})}
				>
					{({ data: commitDetails }) => (
						<Diff
							changes={commitDetails.changes}
							filesVisible={filesVisible}
							filesItems={getCommitFileRowItems({ commitDetails })}
							onFileSelection={selectFile}
							selection={selection}
							projectId={projectId}
						/>
					)}
				</SuspenseQuery>
			</Suspense>
		</div>
	);
};

const BranchDetails: FC<{
	selection: Extract<Operand, { _tag: "Branch" }>;
}> = ({ selection }) => {
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
	const [branchTab, setBranchTab] = useState<BranchTab>("diff");
	const { branchRef } = selection;

	const selectFile = (selection: string) => {
		dispatch(projectSlice.actions.selectFiles({ projectId, selection }));
	};

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

	return (
		<div className={styles.container}>
			<div className={styles.headerWrap}>
				<div className={styles.titleRow}>
					{detailsFullWindow && <TopLeftControls />}

					<SuspenseQuery
						{...branchDetailsQueryOptions({
							projectId,
							...branchDetailsParams(decodeBytes(selection.branchRef)),
						})}
					>
						{({ data: branchDetails }) => (
							<div className={styles.title}>
								<Icon name="branch" />
								<h3 className={classes("text-15", "text-semibold")}>{branchDetails.name}</h3>
							</div>
						)}
					</SuspenseQuery>
				</div>

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
										decodeBytes(selection.branchRef).replace(/^refs\/heads\//, ""),
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
			</div>

			<Suspense fallback={<div className={classes(styles.loadingTab, "text-13")}>Loading…</div>}>
				{branchTab === "pr" ? (
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
						{...branchDiffQueryOptions({ projectId, branch: decodeBytes(selection.branchRef) })}
					>
						{({ data: branchDiff }) => (
							<Diff
								changes={branchDiff.changes}
								filesVisible={filesVisible}
								filesItems={getBranchFileRowItems({ branchDiff })}
								onFileSelection={selectFile}
								selection={selection}
								projectId={projectId}
							/>
						)}
					</SuspenseQuery>
				)}
			</Suspense>
		</div>
	);
};

const FileDetails: FC<{
	selection: Extract<Operand, { _tag: "File" }> & {
		parent: Extract<FileParent, { _tag: "UncommittedChanges" }>;
	};
}> = ({ selection }) => {
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

	const selectFile = (selection: string) => {
		dispatch(projectSlice.actions.selectFiles({ projectId, selection }));
	};

	return (
		<div className={styles.container}>
			<div className={styles.headerWrap}>
				<div className={styles.titleRow}>
					{detailsFullWindow && <TopLeftControls />}

					<div className={styles.title}>
						<Icon name="file" />
						<h3 className={classes("text-15", "text-semibold")}>{selection.path}</h3>
					</div>
				</div>
			</div>

			<Suspense fallback={<div className={classes(styles.loadingTab, "text-13")}>Loading…</div>}>
				<SuspenseQuery {...changesInWorktreeQueryOptions(projectId)}>
					{({ data: worktreeChanges }) => {
						const filesItems = getChangesFileRowItems(worktreeChanges).filter(
							(item) => item.path === selection.path,
						);
						const changes = filesItems.flatMap((item) =>
							item._tag === "Change" ? [item.change] : [],
						);

						if (changes.length === 0) return null;

						return (
							<Diff
								changes={changes}
								filesVisible={filesVisible}
								filesItems={filesItems}
								onFileSelection={selectFile}
								selection={selection}
								projectId={projectId}
							/>
						);
					}}
				</SuspenseQuery>
			</Suspense>
		</div>
	);
};

export const Details: FC<{
	selection: Operand | null;
}> = ({ selection }) => {
	if (!selection) return;

	return Match.value(selection).pipe(
		Match.tags({
			Commit: (commit) => <CommitDetails selection={commit} />,
			Branch: (branch) => <BranchDetails selection={branch} />,
		}),
		Match.when({ _tag: "File", parent: { _tag: "UncommittedChanges" } }, (file) => (
			<FileDetails selection={file} />
		)),
		Match.orElseAbsurd,
	);
};
