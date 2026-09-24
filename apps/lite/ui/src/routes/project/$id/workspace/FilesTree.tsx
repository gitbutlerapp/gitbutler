import rowStyles from "./Row.module.css";
import {
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	listEditorsQueryOptions,
} from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { defaultSettings } from "#ui/settings.ts";
import { fileAddress, addressEquals, addressIdentityKey, type FileParent } from "#ui/addresses.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { getRangeExtractorWithIndices } from "@gitbutler/ui-react/virtual.ts";
import { mergeProps, Tooltip, useRender } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { type Range, useVirtualizer } from "@tanstack/react-virtual";
import {
	type ComponentProps,
	type FC,
	type ReactNode,
	type RefObject,
	useCallback,
	useDeferredValue,
	useLayoutEffect,
	useRef,
	useState,
} from "react";
import styles from "./FilesTree.module.css";
import { Row, RowLabel, RowLabelContainer } from "./Row.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { useAddressSpaceHotkeys, type FocusScope } from "#ui/focus-scopes.ts";
import {
	addressSpaceIncludes,
	getAdjacent,
	type AddressSpace,
} from "#ui/workspace/address-space.ts";
import { changesFileHotkeys } from "#ui/hotkeys.ts";
import { useRevealInFolder } from "./useRevealInFolder.ts";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { FileRow, FileRowPresentational } from "./FileRow.tsx";
import {
	DirectoryRow,
	DirectoryRowPresentational,
	type DirectoryCheckedState,
} from "./DirectoryRow.tsx";
import type { FileRowItem } from "./file-row.ts";
import { parentDirectoryRow, type FileTreeRow } from "./file-tree.ts";
import { useFileDisplayMode } from "./useFileDisplayMode.ts";
import { checkedRange, addressSpaceRange, selectionAfterChecking } from "#ui/checking.ts";
import { useOpenInProgram } from "#ui/api/mutations.ts";
import { useFileSetActions, useFileSetSubject } from "./useFileSetActions.ts";
import type { CSSProperties } from "react";
import { FileRowTooltipRoot, type FileRowTooltipPayload } from "./FileRowTooltip.tsx";

/** One identity for every tree that has no reviewed paths, so the default is stable. */
const EMPTY_REVIEWED_PATHS: ReadonlySet<string> = new Set();

const useFilesTreeHotkeys = ({
	checkAll,
	checkRow,
	addressSpace,
	onRowSelection,
	onEdgeSpill,
	projectId,
	ref,
	fileParent,
	rows,
	selection,
	selectedRow,
	selectedChangePaths,
	toggleDirectoryCollapsed,
}: {
	checkAll: () => void;
	checkRow: (evt: { path: string; shiftKey: boolean }) => string | null;
	addressSpace: AddressSpace<string>;
	onRowSelection: (selection: string) => void;
	onEdgeSpill?: (offset: -1 | 1) => void;
	projectId: string;
	ref: React.RefObject<HTMLElement | null>;
	fileParent: FileParent;
	rows: Array<FileTreeRow<FileRowItem>>;
	selection: string | null;
	selectedRow: FileTreeRow<FileRowItem> | undefined;
	/** What the selected row stands for: its own file, or every file below a directory. */
	selectedChangePaths: Array<string>;
	toggleDirectoryCollapsed: (path: string) => void;
}) => {
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);
	const mode = useFileDisplayMode();
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: preferredEditor } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => editors?.find((editor) => editor.id === cfg.editorId),
	});
	const { mutate: openInProgram } = useOpenInProgram();
	const revealInFolder = useRevealInFolder(projectId);
	const actions = useFileSetActions({ projectId, fileParent });

	const store = useAppStore();

	// As with the other list-wide hotkeys, checked files are the subject when there are any.
	const subject = useFileSetSubject({
		projectId,
		fileParent,
		promote: true,
		own: () => selectedChangePaths.map((path) => fileAddress({ parent: fileParent, path })),
		ownCount: selectedChangePaths.length,
	});
	const hasSelectedChanges = selectedChangePaths.length > 0;

	// Repeats must follow the pending cursor before React renders it. Null ends the held-key run
	// so it cannot reverse and undo the checks; a fresh keypress starts from the selected row.
	const nextCheckedRow = useRef<string>(null);
	const toggleSelectedRowChecked = (event: KeyboardEvent) => {
		if (selection === null) return;
		// Leave activation of a directly focused checkbox to the checkbox itself.
		if (event.target !== ref.current) return;

		event.preventDefault();
		event.stopPropagation();
		if (event.shiftKey) {
			nextCheckedRow.current = null;
			checkRow({ path: selection, shiftKey: true });
			return;
		}
		const item = event.repeat ? nextCheckedRow.current : selection;
		if (item !== null) nextCheckedRow.current = checkRow({ path: item, shiftKey: false });
	};

	const discardSelectedRow = () => {
		if (hasSelectedChanges) void actions.discard(subject.addresses());
	};

	const uncommitSelectedRow = () => {
		if (hasSelectedChanges) actions.uncommit(subject.addresses());
	};

	const absorbSelectedRow = () => {
		if (hasSelectedChanges) actions.absorb(subject.addresses());
	};

	/**
	 * A directory row folds or unfolds itself; a file row folds the directory it
	 * sits in, handing it the selection so the selection stays visible. A file at
	 * the tree root has nothing to fold.
	 */
	const toggleFoldSelectedRow = () => {
		if (selectedRow === undefined) return;

		if (selectedRow._tag === "Directory") {
			toggleDirectoryCollapsed(selectedRow.path);
			return;
		}

		const parent = parentDirectoryRow(
			rows,
			rows.findIndex((row) => row.path === selectedRow.path),
		);
		if (parent === null) return;
		onRowSelection(parent.path);
		toggleDirectoryCollapsed(parent.path);
	};

	const canDiscardSelectedRow = hasSelectedChanges && actions.canDiscard;

	const canCheckTheseFiles = useAppSelector((state) =>
		projectSlice.selectors.selectCanCheckFiles(state, projectId, fileParent),
	);

	useHotkeys([
		{
			hotkey: changesFileHotkeys.checkAll.hotkey,
			callback: checkAll,
			options: {
				conflictBehavior: "allow",
				enabled: selectedRow !== undefined && noOperationPending && canCheckTheseFiles,
				ignoreInputs: true,
				target: ref,
				meta: changesFileHotkeys.checkAll.meta,
			},
		},
		{
			hotkey: changesFileHotkeys.absorb.hotkey,
			callback: absorbSelectedRow,
			options: {
				conflictBehavior: "allow",
				enabled: hasSelectedChanges && actions.canAbsorb && noOperationPending,
				target: ref,
				meta: changesFileHotkeys.absorb.meta,
			},
		},
		{
			hotkey: changesFileHotkeys.checkFile.hotkey,
			callback: toggleSelectedRowChecked,
			options: {
				conflictBehavior: "allow",
				enabled: selection !== null && noOperationPending && canCheckTheseFiles,
				preventDefault: false,
				stopPropagation: false,
				target: ref,
				meta: changesFileHotkeys.checkFile.meta,
			},
		},
		{
			hotkey: changesFileHotkeys.discard.hotkey,
			callback: discardSelectedRow,
			options: {
				conflictBehavior: "allow",
				enabled: noOperationPending && canDiscardSelectedRow,
				target: ref,
				meta: changesFileHotkeys.discard.meta,
			},
		},
		{
			hotkey: "Shift+Space",
			callback: toggleSelectedRowChecked,
			options: {
				conflictBehavior: "allow",
				enabled: selection !== null && noOperationPending && canCheckTheseFiles,
				preventDefault: false,
				stopPropagation: false,
				target: ref,
			},
		},
		{
			hotkey: changesFileHotkeys.openInEditor.hotkey,
			// A row's path is all opening it takes, and an editor takes a directory
			// as readily as a file, so this reaches whatever the cursor is on.
			callback: () => {
				if (!preferredEditor || selection === null) return;

				openInProgram({
					projectId,
					programId: preferredEditor.id,
					path: selection,
					lineNr: null,
				});
			},
			options: {
				conflictBehavior: "allow",
				enabled: preferredEditor && selection !== null,
				target: ref,
				meta: changesFileHotkeys.openInEditor.meta,
			},
		},
		{
			hotkey: changesFileHotkeys.revealInFolder.hotkey,
			// A row's path locates it in the worktree whichever list it came from and
			// whichever kind of row it is, which is all revealing it needs.
			callback: () => {
				if (selection === null) return;
				void revealInFolder(selection);
			},
			options: {
				conflictBehavior: "allow",
				enabled: selection !== null,
				target: ref,
				meta: changesFileHotkeys.revealInFolder.meta,
			},
		},
		{
			hotkey: changesFileHotkeys.uncommit.hotkey,
			callback: uncommitSelectedRow,
			options: {
				conflictBehavior: "allow",
				enabled: noOperationPending && hasSelectedChanges && actions.canUncommit,
				target: ref,
				meta: changesFileHotkeys.uncommit.meta,
			},
		},
		{
			hotkey: changesFileHotkeys.toggleFoldDirectory.hotkey,
			callback: toggleFoldSelectedRow,
			options: {
				conflictBehavior: "allow",
				// Folding is a view operation, so it stays available in every workspace
				// mode. Flat list mode has no directory rows, nothing to fold.
				enabled: mode === "tree" && selectedRow !== undefined,
				target: ref,
				meta: changesFileHotkeys.toggleFoldDirectory.meta,
			},
		},
	]);

	useAddressSpaceHotkeys({
		projectId,
		addressSpace,
		group: "File",
		select: onRowSelection,
		selection,
		ref,
		onEdgeSpill,
		getKey: (path) => path,
		operationSourcesForItem: (path) => {
			const rowIndex = addressSpace.indexByKey.get(path);
			const row = rowIndex === undefined ? undefined : rows[rowIndex];
			const checkedAddresses = projectSlice.selectors.selectCheckedAddresses(
				store.getState(),
				projectId,
			);
			if (row?._tag === "Directory") {
				const sources = row.items.map((item) =>
					fileAddress({ parent: fileParent, path: item.path }),
				);
				return sources.some((source) =>
					checkedAddresses.some((checked) => addressEquals(checked, source)),
				)
					? checkedAddresses
					: sources;
			}

			const address = fileAddress({ parent: fileParent, path });
			return checkedAddresses.length > 0 ? checkedAddresses : [address];
		},
	});
};

// Extracted as a component boundary for the compiler to memo.
const DirectoryOperationSource: FC<
	{
		projectId: string;
		fileParent: FileParent;
		items: Array<FileRowItem>;
	} & Omit<useRender.ComponentProps<"div">, "onDragStart">
> = ({ projectId, fileParent, items, render, ...props }) => (
	<OperationSourceC
		{...props}
		projectId={projectId}
		sources={items.map((item) => fileAddress({ parent: fileParent, path: item.path }))}
		respectChecked
		outline="outside"
		acceptOriginDrop
		render={render}
	/>
);

/**
 * What every row gets from the tree, bundled so a row's props stay few and
 * stable: the bundle only changes identity when one of its members does.
 */
type RowShared = {
	projectId: string;
	fileParent: FileParent;
	focusScope: FocusScope;
	tooltipHandle: Tooltip.Handle<FileRowTooltipPayload>;
	ageBadgeNow: number | null;
	pathDisplay: "lead" | "trail" | "hidden";
	canCheck: boolean;
	anyOperationPending: boolean;
	checkFile: (evt: { path: string; shiftKey: boolean }) => void;
	checkDirectory: (evt: { path: string; checked: boolean }) => void;
	onRowSelection: (selection: string) => void;
	onToggleDirectoryCollapsed: (path: string) => void;
	branchNameByCommitId: (commitId: string) => string | undefined;
	rail: ReactNode | undefined;
};

/**
 * One virtualised row. Its own component so that a re-render of the list
 * leaves the row's element tree cached when nothing about the row changed:
 * the list itself is left uncompiled by its virtualizer, so anything built
 * inline there would be rebuilt, and re-render every row, on every render.
 */
const FilesTreeRow: FC<{
	row: FileTreeRow<FileRowItem>;
	index: number;
	height: number;
	measureElement: (element: HTMLDivElement | null) => void;
	shared: RowShared;
	isSelected: boolean;
	inert: boolean;
	/** A file row's own checked state, or a directory row's aggregate over its files. */
	checkedState: DirectoryCheckedState;
	isReviewed: boolean;
	isCollapsed: boolean;
	/** Whether the selection sits on or under this directory row. */
	holdsSelection: boolean;
	/** See `renderInteractiveRows` in {@link FilesTreeVirtualList}. */
	interactive: boolean;
}> = ({
	row,
	index,
	height,
	measureElement,
	shared,
	isSelected,
	inert,
	checkedState,
	isReviewed,
	isCollapsed,
	holdsSelection,
	interactive,
}) => {
	const {
		projectId,
		fileParent,
		focusScope,
		tooltipHandle,
		ageBadgeNow,
		pathDisplay,
		canCheck,
		anyOperationPending,
		checkFile,
		checkDirectory,
		onRowSelection,
		onToggleDirectoryCollapsed,
		branchNameByCommitId,
		rail,
	} = shared;
	const virtStyle: CSSProperties = { position: "absolute", top: 0, left: 0, width: "100%", height };

	if (row._tag === "Directory") {
		// As for file rows: the cheap half while scrolling, the one that
		// resolves a menu once the list settles.
		const DirectoryRowComponent = interactive ? DirectoryRow : ScrollingDirectoryRow;
		const directoryRow = (
			<DirectoryRowComponent
				projectId={projectId}
				fileParent={fileParent}
				path={row.path}
				name={row.name}
				items={row.items}
				depth={row.depth}
				isCollapsed={isCollapsed}
				scrollSelectedIntoView={false}
				onToggleCollapsed={() => {
					// Collapsing over the selection hides it, and it would
					// fall back to the first row: hand it to the directory
					// row, as the z hotkey does. Other toggles leave the
					// selection (and the details pane it drives) alone.
					if (!isCollapsed && holdsSelection) onRowSelection(row.path);
					onToggleDirectoryCollapsed(row.path);
				}}
				isSelected={isSelected}
				canCheck={canCheck}
				anyOperationPending={anyOperationPending}
				checkedState={checkedState}
				isReviewed={isReviewed}
				checkDirectory={checkDirectory}
				focusScope={focusScope}
				tooltipHandle={tooltipHandle}
				rail={rail}
				inert={inert}
				onSelect={() => onRowSelection(row.path)}
			/>
		);

		return (
			<TreeItem
				data-index={index}
				ref={measureElement}
				row={row}
				isSelected={isSelected}
				isExpanded={!isCollapsed}
				aria-label={`Directory ${row.path}`}
				style={virtStyle}
				render={
					interactive ? (
						<DirectoryOperationSource
							projectId={projectId}
							fileParent={fileParent}
							items={row.items}
							render={directoryRow}
						/>
					) : (
						directoryRow
					)
				}
			/>
		);
	}

	const item = row.item;
	const address = fileAddress({ parent: fileParent, path: row.path });
	const isChecked = checkedState === "checked";

	return (
		<TreeItem
			data-index={index}
			ref={measureElement}
			row={row}
			isSelected={isSelected}
			style={virtStyle}
			aria-label={
				item._tag === "Change"
					? `${item.change.status.type} ${item.change.path}`
					: `Conflict ${item.path}`
			}
			render={
				interactive ? (
					<OperationSourceC
						projectId={projectId}
						sources={[address]}
						respectChecked
						outline="outside"
						acceptOriginDrop
						render={
							<FileRow
								item={item}
								depth={row.depth}
								pathDisplay={pathDisplay}
								inert={inert}
								isSelected={isSelected}
								scrollSelectedIntoView={false}
								isChecked={isChecked}
								isReviewed={isReviewed}
								onSelect={() => onRowSelection(row.path)}
								canCheck={canCheck && item._tag === "Change"}
								checkFile={checkFile}
								projectId={projectId}
								fileParent={fileParent}
								focusScope={focusScope}
								tooltipHandle={tooltipHandle}
								ageBadgeNow={ageBadgeNow}
								rail={rail}
								branchNameByCommitId={branchNameByCommitId}
							/>
						}
					/>
				) : (
					<FileRowPresentational
						item={item}
						depth={row.depth}
						pathDisplay={pathDisplay}
						inert={inert}
						isSelected={isSelected}
						scrollSelectedIntoView={false}
						isChecked={isChecked}
						isReviewed={isReviewed}
						onSelect={() => onRowSelection(row.path)}
						canCheck={canCheck && item._tag === "Change"}
						checkFile={() => {}}
						projectId={projectId}
						fileParent={fileParent}
						focusScope={focusScope}
						tooltipHandle={tooltipHandle}
						ageBadgeNow={ageBadgeNow}
						rail={rail}
						branchNameByCommitId={() => undefined}
						anyOperationPending={anyOperationPending}
						menuItems={[]}
						presentationalOnly
					/>
				)
			}
		/>
	);
};

/**
 * The virtualised rows. Kept apart from {@link FilesTree} because React
 * Compiler leaves any component calling `useVirtualizer` uncompiled
 * (https://github.com/TanStack/virtual/issues/1119), so this one holds as
 * little as possible: everything the rows need is derived, and memoised, in
 * the tree and arrives here already stable.
 */
const FilesTreeVirtualList: FC<{
	rows: Array<FileTreeRow<FileRowItem>>;
	addressSpace: AddressSpace<string>;
	selection: string | null;
	hasPendingOperationSources: boolean;
	collapsedDirectories: Record<string, true>;
	reviewedPaths: ReadonlySet<string>;
	isFileChecked: (path: string) => boolean;
	directoryCheckedState: (items: Array<FileRowItem>) => DirectoryCheckedState;
	directoryReviewed: (items: Array<FileRowItem>) => boolean;
	shared: RowShared;
	scrollElementRef: RefObject<HTMLElement | null> | undefined;
	scrollMargin: number;
	scrollPaddingStart: number;
	scrollPaddingEnd: number;
}> = ({
	rows,
	addressSpace,
	selection,
	hasPendingOperationSources,
	collapsedDirectories,
	reviewedPaths,
	isFileChecked,
	directoryCheckedState,
	directoryReviewed,
	shared,
	scrollElementRef,
	scrollMargin,
	scrollPaddingStart,
	scrollPaddingEnd,
}) => {
	const selectedRowIndex =
		selection !== null ? (addressSpace.indexByKey.get(selection) ?? null) : null;
	const rangeExtractorWithSelected = useCallback(
		(range: Range) =>
			getRangeExtractorWithIndices(range, selectedRowIndex === null ? [] : [selectedRowIndex]),
		[selectedRowIndex],
	);

	// The list scrolls in the tree's parent, reached from this component's own
	// element: the tree's ref belongs to the parent component and is not attached
	// yet when the virtualizer first asks, which would leave the list empty until
	// something else re-rendered it.
	const groupRef = useRef<HTMLDivElement>(null);
	// oxlint-disable-next-line react-hooks-js/incompatible-library -- https://github.com/TanStack/virtual/issues/1119#issuecomment-4648268095
	const rowVirtualizer = useVirtualizer({
		directDomUpdates: true,
		directDomUpdatesMode: "transform",
		count: rows.length,
		getScrollElement: () =>
			scrollElementRef?.current ?? groupRef.current?.parentElement?.parentElement ?? null,
		// Keep in sync with --single-line-row-height.
		estimateSize: () => 28,
		getItemKey: (index) => rows[index]?.path ?? index,
		rangeExtractor: rangeExtractorWithSelected,
		scrollMargin,
		scrollPaddingStart,
		scrollPaddingEnd,
	});
	const deferredIsScrolling = useDeferredValue(rowVirtualizer.isScrolling, true);
	// Keep OperationSourceC mounted while an operation refers to its rows, especially while a
	// pointer transfer auto-scrolls. Otherwise render the cheap rows immediately on scroll and
	// wait for deferredIsScrolling to catch up before upgrading them in an interruptible render.
	const renderInteractiveRows =
		hasPendingOperationSources || (!rowVirtualizer.isScrolling && !deferredIsScrolling);

	// Virtualisation-friendly equivalent to Row's own scrollIntoView. Again as
	// the head and foot to clear are measured or grow; auto is a no-op once clear.
	useLayoutEffect(() => {
		if (selectedRowIndex !== null)
			rowVirtualizer.scrollToIndex(selectedRowIndex, { align: "auto" });
	}, [rowVirtualizer, selectedRowIndex, scrollPaddingStart, scrollPaddingEnd]);

	return (
		<div
			// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- Tree items need ARIA group semantics.
			role="group"
			ref={useMergedRefs(rowVirtualizer.containerRef, groupRef)}
			style={{ position: "relative" }}
		>
			{rowVirtualizer.getVirtualItems().map((virtualRow) => {
				const row = rows[virtualRow.index];
				if (row === undefined) return null;

				const isDirectory = row._tag === "Directory";
				return (
					<FilesTreeRow
						key={row.path}
						row={row}
						index={virtualRow.index}
						height={virtualRow.size}
						measureElement={rowVirtualizer.measureElement}
						shared={shared}
						isSelected={selection !== null && selection === row.path}
						inert={!addressSpaceIncludes(addressSpace, row.path, (path) => path)}
						checkedState={
							isDirectory
								? directoryCheckedState(row.items)
								: isFileChecked(row.path)
									? "checked"
									: "unchecked"
						}
						isReviewed={
							isDirectory
								? directoryReviewed(row.items)
								: row.item._tag === "Change" && reviewedPaths.has(row.item.change.path)
						}
						isCollapsed={isDirectory && collapsedDirectories[row.path] === true}
						holdsSelection={
							isDirectory &&
							selection !== null &&
							(selection === row.path || selection.startsWith(`${row.path}/`))
						}
						interactive={renderInteractiveRows}
					/>
				);
			})}
		</div>
	);
};

export const FilesTree: FC<
	{
		projectId: string;
		rows: Array<FileTreeRow<FileRowItem>>;
		collapsedDirectories: Record<string, true>;
		onToggleDirectoryCollapsed: (path: string) => void;
		selection: string | null;
		onRowSelection: (selection: string) => void;
		/** See {@link useAddressSpaceHotkeys}'s option of the same name. */
		onEdgeSpill?: (offset: -1 | 1) => void;
		addressSpace: AddressSpace<string>;
		fileParent: FileParent;
		/** The scope this tree's hotkeys are bound to; also stamped on the tree element. */
		focusScope: FocusScope;
		/**
		 * Paths whose diff, as it currently stands, has been reviewed. Those rows
		 * report it in place of their change type. Empty where reviewing does not
		 * apply, which hides the mark.
		 */
		reviewedPaths?: ReadonlySet<string>;
		/**
		 * Timestamp the row age badges are measured against; `null` hides them.
		 * The caller owns the ticking.
		 */
		ageBadgeNow?: number | null;
		/** On the graph, every row's rail, drawn before its steps. */
		rail?: ReactNode;
		/** The scroller the list scrolls in when it is not the tree's own parent, and the list's offset in it. */
		scrollElementRef?: RefObject<HTMLElement | null>;
		scrollMargin?: number;
		/** Room a row scrolled into view keeps clear at the scroller's head and foot; by default, its gradients. */
		scrollPaddingStart?: number;
		scrollPaddingEnd?: number;
	} & ComponentProps<"div">
> = ({
	rows,
	collapsedDirectories,
	onToggleDirectoryCollapsed,
	selection,
	onRowSelection,
	onEdgeSpill,
	projectId,
	addressSpace,
	fileParent,
	focusScope,
	reviewedPaths = EMPTY_REVIEWED_PATHS,
	ageBadgeNow = null,
	rail,
	scrollElementRef,
	scrollMargin = 0,
	scrollPaddingStart = 14,
	scrollPaddingEnd = 14,
	ref: refProp,
	...props
}) => {
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});
	// Resolved once here rather than per row: a row that subscribes to the settings query
	// is a row that re-renders with it. Selecting the boolean keeps that subscription to
	// this one field.
	const { data: pathFirst } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => cfg.pathFirst ?? defaultSettings.pathFirst,
	});
	const mode = useFileDisplayMode();
	const canCheck = useAppSelector((state) =>
		projectSlice.selectors.selectCanCheckFiles(state, projectId, fileParent),
	);
	const pendingOperationTag = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag,
	);
	const anyOperationPending = pendingOperationTag !== "None";
	const hasPendingOperationSources =
		pendingOperationTag === "Absorb" || pendingOperationTag === "Transfer";
	const checkedAddressKeys = useAppSelector((state) =>
		projectSlice.selectors.selectCheckedAddressKeys(state, projectId),
	);
	const store = useAppStore();
	const dispatch = useAppDispatch();
	// Create once per tree: rows in separate trees can have the same DOM ID, but a tooltip store
	// can register only one element for each ID.
	const [tooltipHandle] = useState(() => Tooltip.createHandle<FileRowTooltipPayload>());

	const ref = useRef<HTMLDivElement>(null);

	const fileCheckRangeAnchor = useRef<string>(null);
	const fileCheckRangeEnd = useRef<string>(null);
	const rowByPath = new Map(rows.map((row) => [row.path, row]));
	// Conflicts have no change to commit or discard yet, so they never get checked.
	const conflictPaths = new Set(
		rows
			.values()
			.filter((row) => row._tag === "File" && row.item._tag === "Conflict")
			.map((row) => row.path),
	);
	const checkable = (path: string) => !conflictPaths.has(path);
	const selectedRow = selection === null ? undefined : rowByPath.get(selection);
	// A directory row stands for every file below it, so the list hotkeys reach a whole
	// subtree the way its menu does. Conflicts have no change to act on either way.
	const selectedChangePaths = changePathsOfRow(selectedRow);

	// The tree already names the directory a row sits under, so repeating it on
	// the row itself would say it twice.
	const pathDisplay =
		mode === "tree" ? "hidden" : (pathFirst ?? defaultSettings.pathFirst) ? "lead" : "trail";

	const isFileChecked = (path: string): boolean =>
		checkedAddressKeys.has(addressIdentityKey(fileAddress({ parent: fileParent, path })));

	const directoryCheckedState = (items: Array<FileRowItem>): DirectoryCheckedState => {
		const checkableItems = items.filter((item) => checkable(item.path));
		const checkedCount = checkableItems.filter((item) => isFileChecked(item.path)).length;
		if (checkedCount === 0) return "unchecked";
		return checkedCount === checkableItems.length ? "checked" : "indeterminate";
	};

	// A directory is reviewed once every change below it is — the same "all of
	// them" its checkbox reads by. Conflicts have no diff to review, so, as with
	// checking, they don't count; a directory holding nothing else is not reviewed.
	const directoryReviewed = (items: Array<FileRowItem>): boolean => {
		const changePaths = items.filter((item) => item._tag === "Change").map((item) => item.path);
		return changePaths.length > 0 && changePaths.every((path) => reviewedPaths.has(path));
	};

	const rangeResolver = addressSpaceRange<string, string>({
		addressSpace,
		getKey: (path) => path,
		// Range-checking runs over files; a directory caught in the middle of a
		// range is passed over rather than checked as a path of its own.
		filterMap: (path) => (rowByPath.get(path)?._tag === "Directory" ? null : path),
	});
	const getCheckedRange = checkedRange(rangeResolver);

	const checkedFilePaths = (): Set<string> => {
		const checkedAddresses = projectSlice.selectors.selectCheckedAddresses(
			store.getState(),
			projectId,
		);
		return new Set(
			checkedAddresses
				.values()
				.map((address) =>
					address._tag === "File" && addressEquals(address.parent, fileParent)
						? address.path
						: null,
				)
				.filter((x) => x != null),
		);
	};

	const applyCheckedFiles = ({
		previous,
		next,
	}: {
		previous: Set<string>;
		next: Set<string>;
	}): void => {
		const nextCheckable = new Set([...next].filter(checkable));
		const addresses = (paths: Set<string>) =>
			Array.from(paths, (path) => fileAddress({ parent: fileParent, path }));

		dispatch(
			projectSlice.actions.checkAddresses({
				projectId,
				addresses: addresses(nextCheckable.difference(previous)),
				checked: true,
			}),
		);
		dispatch(
			projectSlice.actions.checkAddresses({
				projectId,
				addresses: addresses(previous.difference(nextCheckable)),
				checked: false,
			}),
		);
	};

	const checkFile = ({ path, shiftKey }: { path: string; shiftKey: boolean }): void => {
		const previous = checkedFilePaths();
		const nextFileRange = getCheckedRange({
			checked: previous,
			rangeAnchor: fileCheckRangeAnchor.current,
			rangeEnd: fileCheckRangeEnd.current,
		})({
			item: path,
			shiftKey,
		});

		fileCheckRangeAnchor.current = nextFileRange.rangeAnchor;
		fileCheckRangeEnd.current = nextFileRange.rangeEnd;

		applyCheckedFiles({ previous, next: nextFileRange.checked });
	};

	const checkDirectory = ({ path, checked }: { path: string; checked: boolean }): void => {
		const row = rowByPath.get(path);
		if (row?._tag !== "Directory") return;

		// A directory stands for a set rather than a point, so it can't anchor a
		// range the way a file does.
		fileCheckRangeAnchor.current = null;
		fileCheckRangeEnd.current = null;

		const previous = checkedFilePaths();
		const subject = new Set(row.items.map((item) => item.path));
		applyCheckedFiles({
			previous,
			next: checked ? previous.union(subject) : previous.difference(subject),
		});
	};

	/** Keyboard checking for either kind of row. */
	const checkRow = ({ path, shiftKey }: { path: string; shiftKey: boolean }): string | null => {
		const row = rowByPath.get(path);
		if (row?._tag === "Directory") {
			const checked = checkedFilePaths();
			checkDirectory({
				path,
				checked: !row.items
					.filter((item) => checkable(item.path))
					.every((item) => checked.has(item.path)),
			});
		} else {
			if (!row || !checkable(path)) return null;
			checkFile({ path, shiftKey });
		}

		if (shiftKey) return null;
		const checked = checkedFilePaths();
		const next = selectionAfterChecking({
			selection: path,
			getAdjacent: (offset) =>
				getAdjacent({ addressSpace, selection: path, offset, getKey: (path) => path }),
			getChecked: (path) => {
				const row = rowByPath.get(path);
				if (!row || !checkable(path)) return null;
				if (row._tag === "File") return checked.has(path);
				const checkableItems = row.items.filter((item) => checkable(item.path));
				return checkableItems.length > 0
					? checkableItems.every((item) => checked.has(item.path))
					: null;
			},
		});
		if (next !== null) onRowSelection(next);
		return next;
	};

	useFilesTreeHotkeys({
		checkAll: () => {
			if (!selectedRow) return;

			const lastSepIdx = selectedRow.path.lastIndexOf("/");
			const directoryPath =
				selectedRow._tag === "Directory"
					? selectedRow.path
					: lastSepIdx === -1
						? ""
						: selectedRow.path.slice(0, lastSepIdx);
			const dir = rowByPath.get(directoryPath);
			if (dir?._tag === "Directory") return checkDirectory({ path: dir.path, checked: true });

			const prefix = directoryPath === "" ? "" : `${directoryPath}/`;
			const paths = rows
				.values()
				.filter((row) => row.depth === 0)
				.flatMap((row) =>
					row._tag === "Directory" ? row.items.map((item) => item.path) : row.path,
				);

			const previous = checkedFilePaths();
			fileCheckRangeAnchor.current = null;
			fileCheckRangeEnd.current = null;

			applyCheckedFiles({
				previous,
				next: previous.union(new Set(paths.filter((path) => path.startsWith(prefix)))),
			});
		},
		checkRow,
		addressSpace,
		onRowSelection,
		onEdgeSpill,
		projectId,
		ref,
		fileParent,
		rows,
		selection,
		selectedRow,
		selectedChangePaths,
		toggleDirectoryCollapsed: onToggleDirectoryCollapsed,
	});

	const shared: RowShared = {
		projectId,
		fileParent,
		focusScope,
		tooltipHandle,
		ageBadgeNow,
		pathDisplay,
		canCheck,
		anyOperationPending,
		checkFile,
		checkDirectory,
		onRowSelection,
		onToggleDirectoryCollapsed,
		branchNameByCommitId: (commitId) =>
			headInfoIndex?.commitContextByCommitId(commitId)?.segment.refName?.displayName,
		rail,
	};

	return (
		<div
			{...props}
			data-focus-scope={focusScope}
			tabIndex={0}
			role="tree"
			aria-activedescendant={selection !== null ? treeItemId(selection) : undefined}
			className={classes(props.className, styles.tree)}
			ref={useMergedRefs(refProp, ref)}
		>
			<FileRowTooltipRoot handle={tooltipHandle} />
			{rows.length === 0 ? (
				<Row interactive={false}>
					{rail}
					<RowLabelContainer>
						{/* Both callers hide the tree outright when there is nothing to
						    list, so an empty tree only ever means the filter matched
						    nothing. */}
						<RowLabel className={rowStyles.fadedText}>No matching files.</RowLabel>
					</RowLabelContainer>
				</Row>
			) : (
				<FilesTreeVirtualList
					rows={rows}
					addressSpace={addressSpace}
					selection={selection}
					hasPendingOperationSources={hasPendingOperationSources}
					collapsedDirectories={collapsedDirectories}
					reviewedPaths={reviewedPaths}
					isFileChecked={isFileChecked}
					directoryCheckedState={directoryCheckedState}
					directoryReviewed={directoryReviewed}
					shared={shared}
					scrollElementRef={scrollElementRef}
					scrollMargin={scrollMargin}
					scrollPaddingStart={scrollPaddingStart}
					scrollPaddingEnd={scrollPaddingEnd}
				/>
			)}
		</div>
	);
};

/** The changes a row stands for: its own, or every one below a directory. */
const changePathsOfRow = (row: FileTreeRow<FileRowItem> | undefined): Array<string> => {
	if (row === undefined) return [];
	if (row._tag === "Directory")
		return row.items.filter((item) => item._tag === "Change").map((item) => item.path);

	return row.item._tag === "Change" ? [row.path] : [];
};

const treeItemId = (path: string): string => `files-treeitem-${encodeURIComponent(path)}`;

/**
 * A directory row as it renders mid-scroll: the settled row's shape with no menu
 * resolved behind it, which is what {@link FileRowPresentational} does with the
 * same props.
 */
const ScrollingDirectoryRow: FC<ComponentProps<typeof DirectoryRow>> = ({
	projectId: _projectId,
	fileParent: _fileParent,
	...props
}) => <DirectoryRowPresentational {...props} menuItems={[]} presentationalOnly />;

/**
 * One row of the flattened tree. Depth is reported rather than nested, which is
 * what `aria-level` and its siblings are for: a screen reader still hears the
 * shape a `role="group"` per directory would have given it.
 */
const TreeItem: FC<
	{
		row: FileTreeRow<FileRowItem>;
		isSelected: boolean;
		isExpanded?: boolean;
	} & useRender.ComponentProps<"div">
> = ({ row, isSelected, isExpanded, render, ...props }) =>
	useRender({
		render,
		defaultTagName: "div",
		props: mergeProps<"div">(props, {
			id: treeItemId(row.path),
			role: "treeitem",
			"aria-selected": isSelected,
			"aria-expanded": isExpanded,
			"aria-level": row.depth + 1,
			"aria-posinset": row.positionInSet,
			"aria-setsize": row.setSize,
		}),
	});
