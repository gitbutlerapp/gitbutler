import rowStyles from "./Row.module.css";
import { enterAbsorb } from "#ui/use-cursor.ts";
import {
	guiSettingsQueryOptions,
	headInfoQueryOptions,
	listEditorsQueryOptions,
} from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { defaultSettings } from "#ui/settings.ts";
import {
	uncommittedChangesFileParent,
	fileOperand,
	operandEquals,
	operandIdentityKey,
	type FileParent,
} from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { mergeProps, useRender } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import { type ComponentProps, type FC, useRef } from "react";
import styles from "./FilesTree.module.css";
import { Row, RowLabel, RowLabelContainer } from "./Row.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import {
	focusSelectionScope,
	useNavigationIndexHotkeys,
	type SelectionScope,
} from "#ui/selection-scopes.ts";
import { navigationIndexIncludes, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { changesFileHotkeys } from "#ui/hotkeys.ts";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { FileRow } from "./FileRow.tsx";
import { DirectoryRow, type DirectoryCheckedState } from "./DirectoryRow.tsx";
import type { FileRowItem } from "./file-row.ts";
import { parentDirectoryRow, type FileTreeRow } from "./file-tree.ts";
import { useFileDisplayMode } from "./useFileDisplayMode.ts";
import { checkedRange, navigationIndexRange } from "#ui/checking.ts";
import { useDiscardFileChanges, useOpenInProgram } from "#ui/api/mutations.ts";
import type { TreeChange } from "@gitbutler/but-sdk";

const useFilesTreeHotkeys = ({
	checkRow,
	navigationIndex,
	onRowSelection,
	onEdgeSpill,
	projectId,
	ref,
	fileParent,
	canUncommit,
	uncommit,
	rows,
	selection,
	selectedRow,
	selectedChange,
	toggleDirectoryCollapsed,
}: {
	checkRow: (evt: { path: string; shiftKey: boolean }) => void;
	navigationIndex: NavigationIndex<string>;
	onRowSelection: (selection: string) => void;
	onEdgeSpill?: (offset: -1 | 1) => void;
	projectId: string;
	ref: React.RefObject<HTMLElement | null>;
	fileParent: FileParent;
	canUncommit: boolean;
	uncommit?: (change: TreeChange, extendToCheckedFiles: boolean) => void;
	rows: Array<FileTreeRow<FileRowItem>>;
	selection: string | null;
	selectedRow: FileTreeRow<FileRowItem> | undefined;
	selectedChange: TreeChange | null;
	toggleDirectoryCollapsed: (path: string) => void;
}) => {
	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);
	const mode = useFileDisplayMode();
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: preferredEditor } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => editors?.find((editor) => editor.id === cfg.editorId),
	});
	const { mutate: openInProgram } = useOpenInProgram();
	const { canDiscard, discard } = useDiscardFileChanges({ projectId, fileParent });

	const store = useAppStore();

	const selectedChangesFile = fileParent._tag === "UncommittedChanges" ? selection : null;
	const selectedUncommittedChange =
		fileParent._tag === "UncommittedChanges" ? selectedChange : null;

	const absorbSelectedFile = () => {
		if (selectedUncommittedChange === null) return;

		enterAbsorb({
			source: fileOperand({
				parent: uncommittedChangesFileParent,
				path: selectedUncommittedChange.path,
			}),
			sourceTarget: {
				type: "treeChanges",
				subject: {
					changes: [selectedUncommittedChange],
					assignedStackId: null,
				},
			},
		});
		focusSelectionScope("outline");
	};

	const toggleSelectedRowChecked = (event: KeyboardEvent) => {
		if (selection === null) return;
		// Leave activation of a directly focused checkbox to the checkbox itself.
		if (event.target !== ref.current) return;

		event.preventDefault();
		event.stopPropagation();
		checkRow({ path: selection, shiftKey: event.shiftKey });
	};

	const discardSelectedFile = () => {
		if (selectedChange === null) return;

		// As with the other list-wide hotkeys, checked files are the subject when there are any.
		void discard({ change: selectedChange, extendToCheckedFiles: true });
	};

	const uncommitSelectedFile = () => {
		if (selectedChange === null || fileParent._tag !== "Commit") return;

		uncommit?.(selectedChange, true);
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

	const canDiscardSelectedFile = selectedChange !== null && canDiscard;

	const canCheckTheseFiles = useAppSelector((state) =>
		projectSlice.selectors.selectCanCheckFiles(state, projectId, fileParent),
	);

	useHotkeys([
		{
			hotkey: changesFileHotkeys.absorb.hotkey,
			callback: absorbSelectedFile,
			options: {
				conflictBehavior: "allow",
				enabled: selectedChangesFile !== null && isDefaultMode,
				target: ref,
				meta: changesFileHotkeys.absorb.meta,
			},
		},
		{
			hotkey: changesFileHotkeys.checkFile.hotkey,
			callback: toggleSelectedRowChecked,
			options: {
				conflictBehavior: "allow",
				enabled: selection !== null && isDefaultMode && canCheckTheseFiles,
				preventDefault: false,
				stopPropagation: false,
				target: ref,
				meta: changesFileHotkeys.checkFile.meta,
			},
		},
		{
			hotkey: changesFileHotkeys.discard.hotkey,
			callback: discardSelectedFile,
			options: {
				conflictBehavior: "allow",
				enabled: isDefaultMode && canDiscardSelectedFile,
				target: ref,
				meta: changesFileHotkeys.discard.meta,
			},
		},
		{
			hotkey: "Shift+Space",
			callback: toggleSelectedRowChecked,
			options: {
				conflictBehavior: "allow",
				enabled: selection !== null && isDefaultMode && canCheckTheseFiles,
				preventDefault: false,
				stopPropagation: false,
				target: ref,
			},
		},
		{
			hotkey: changesFileHotkeys.openInEditor.hotkey,
			callback: () => {
				if (!preferredEditor || selectedChangesFile === null) return;

				openInProgram({
					projectId,
					programId: preferredEditor.id,
					path: selectedChangesFile,
					lineNr: null,
				});
			},
			options: {
				conflictBehavior: "allow",
				enabled: preferredEditor && selectedChangesFile !== null,
				target: ref,
				meta: changesFileHotkeys.openInEditor.meta,
			},
		},
		{
			hotkey: changesFileHotkeys.uncommit.hotkey,
			callback: uncommitSelectedFile,
			options: {
				conflictBehavior: "allow",
				enabled:
					isDefaultMode && selectedChange !== null && fileParent._tag === "Commit" && canUncommit,
				target: ref,
				meta: changesFileHotkeys.uncommit.meta,
			},
		},
		{
			hotkey: changesFileHotkeys.toggleFoldDirectory.hotkey,
			callback: toggleFoldSelectedRow,
			options: {
				conflictBehavior: "allow",
				// Folding is a view operation, so it stays available in every outline
				// mode. Flat list mode has no directory rows, nothing to fold.
				enabled: mode === "tree" && selectedRow !== undefined,
				target: ref,
				meta: changesFileHotkeys.toggleFoldDirectory.meta,
			},
		},
	]);

	useNavigationIndexHotkeys({
		navigationIndex,
		group: "File",
		select: onRowSelection,
		selection,
		ref,
		onEdgeSpill,
		getKey: (path) => path,
		operationSourcesForItem: (path) => {
			const operand = fileOperand({ parent: fileParent, path });
			const checkedOperands = projectSlice.selectors.selectCheckedOperands(
				store.getState(),
				projectId,
			);
			return checkedOperands.length > 0 ? checkedOperands : [operand];
		},
	});
};

export const FilesTree: FC<
	{
		projectId: string;
		rows: Array<FileTreeRow<FileRowItem>>;
		canUncommit: boolean;
		uncommit?: (change: TreeChange, extendToCheckedFiles: boolean) => void;
		collapsedDirectories: Record<string, true>;
		onToggleDirectoryCollapsed: (path: string) => void;
		selection: string | null;
		onRowSelection: (selection: string) => void;
		/** See {@link useNavigationIndexHotkeys}'s option of the same name. */
		onEdgeSpill?: (offset: -1 | 1) => void;
		navigationIndex: NavigationIndex<string>;
		fileParent: FileParent;
		/** The scope this tree's hotkeys are bound to; also stamped on the tree element. */
		selectionScope: SelectionScope;
		emptyLabel?: string;
	} & ComponentProps<"div">
> = ({
	rows,
	canUncommit,
	uncommit,
	collapsedDirectories,
	onToggleDirectoryCollapsed,
	selection,
	onRowSelection,
	onEdgeSpill,
	projectId,
	navigationIndex,
	fileParent,
	selectionScope,
	emptyLabel = "No changes.",
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
	const checkedOperandKeys = useAppSelector((state) =>
		projectSlice.selectors.selectCheckedOperandKeys(state, projectId),
	);
	const store = useAppStore();
	const dispatch = useAppDispatch();

	const ref = useRef<HTMLDivElement>(null);

	const fileCheckRangeAnchor = useRef<string>(null);
	const fileCheckRangeEnd = useRef<string>(null);
	const rowByPath = new Map(rows.map((row) => [row.path, row]));
	const selectedRow = selection === null ? undefined : rowByPath.get(selection);
	const selectedItem = selectedRow?._tag === "File" ? selectedRow.item : undefined;
	const selectedChange = selectedItem?._tag === "Change" ? selectedItem.change : null;

	// The tree already names the directory a row sits under, so repeating it on
	// the row itself would say it twice.
	const pathDisplay =
		mode === "tree" ? "hidden" : (pathFirst ?? defaultSettings.pathFirst) ? "lead" : "trail";

	const isFileChecked = (path: string): boolean =>
		checkedOperandKeys.has(operandIdentityKey(fileOperand({ parent: fileParent, path })));

	const directoryCheckedState = (filePaths: Array<string>): DirectoryCheckedState => {
		const checkedCount = filePaths.filter((path) => isFileChecked(path)).length;
		if (checkedCount === 0) return "unchecked";
		return checkedCount === filePaths.length ? "checked" : "indeterminate";
	};

	const rangeResolver = navigationIndexRange<string, string>({
		navigationIndex,
		getKey: (path) => path,
		// Range-checking runs over files; a directory caught in the middle of a
		// range is passed over rather than checked as a path of its own.
		filterMap: (path) => (rowByPath.get(path)?._tag === "Directory" ? null : path),
	});
	const getCheckedRange = checkedRange(rangeResolver);

	const checkedFilePaths = (): Set<string> => {
		const checkedOperands = projectSlice.selectors.selectCheckedOperands(
			store.getState(),
			projectId,
		);
		return new Set(
			checkedOperands.flatMap((operand) =>
				operand._tag === "File" && operandEquals(operand.parent, fileParent) ? operand.path : [],
			),
		);
	};

	const applyCheckedFiles = ({
		previous,
		next,
	}: {
		previous: Set<string>;
		next: Set<string>;
	}): void => {
		const operands = (paths: Set<string>) =>
			Array.from(paths, (path) => fileOperand({ parent: fileParent, path }));

		dispatch(
			projectSlice.actions.checkOperands({
				projectId,
				operands: operands(next.difference(previous)),
				checked: true,
			}),
		);
		dispatch(
			projectSlice.actions.checkOperands({
				projectId,
				operands: operands(previous.difference(next)),
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
		const subject = new Set(row.filePaths);
		applyCheckedFiles({
			previous,
			next: checked ? previous.union(subject) : previous.difference(subject),
		});
	};

	/** Space and the row checkboxes both land here, whichever kind of row it is. */
	const checkRow = ({ path, shiftKey }: { path: string; shiftKey: boolean }): void => {
		const row = rowByPath.get(path);
		if (row?._tag === "Directory") {
			checkDirectory({ path, checked: directoryCheckedState(row.filePaths) !== "checked" });
			return;
		}

		checkFile({ path, shiftKey });
	};

	useFilesTreeHotkeys({
		checkRow,
		navigationIndex,
		onRowSelection,
		onEdgeSpill,
		projectId,
		ref,
		fileParent,
		canUncommit,
		uncommit,
		rows,
		selection,
		selectedRow,
		selectedChange,
		toggleDirectoryCollapsed: onToggleDirectoryCollapsed,
	});

	return (
		<div
			{...props}
			data-selection-scope={selectionScope}
			tabIndex={0}
			role="tree"
			aria-activedescendant={selection !== null ? treeItemId(selection) : undefined}
			className={classes(props.className, styles.tree)}
			ref={useMergedRefs(refProp, ref)}
		>
			{rows.length === 0 ? (
				<Row interactive={false}>
					<RowLabelContainer>
						<RowLabel className={rowStyles.fadedText}>{emptyLabel}</RowLabel>
					</RowLabelContainer>
				</Row>
			) : (
				// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- Tree items need ARIA group semantics.
				<div role="group">
					{rows.map((row) => {
						const isSelected = selection !== null && selection === row.path;

						if (row._tag === "Directory") {
							const isCollapsed = collapsedDirectories[row.path] === true;

							return (
								<TreeItem
									key={row.path}
									row={row}
									isSelected={isSelected}
									isExpanded={!isCollapsed}
									aria-label={`Directory ${row.path}`}
									render={
										<DirectoryRow
											projectId={projectId}
											path={row.path}
											name={row.name}
											fileCount={row.filePaths.length}
											depth={row.depth}
											isCollapsed={isCollapsed}
											onToggleCollapsed={() => {
												// Collapsing over the selection hides it, and it would
												// fall back to the first row: hand it to the directory
												// row, as the z hotkey does. Other toggles leave the
												// selection (and the details pane it drives) alone.
												if (
													!isCollapsed &&
													selection !== null &&
													(selection === row.path || selection.startsWith(`${row.path}/`))
												)
													onRowSelection(row.path);
												onToggleDirectoryCollapsed(row.path);
											}}
											isSelected={isSelected}
											canCheck={canCheck}
											checkedState={directoryCheckedState(row.filePaths)}
											checkDirectory={checkDirectory}
											selectionScope={selectionScope}
											onSelect={() => onRowSelection(row.path)}
										/>
									}
								/>
							);
						}

						const item = row.item;
						const operand = fileOperand({ parent: fileParent, path: row.path });

						return (
							<TreeItem
								key={row.path}
								row={row}
								isSelected={isSelected}
								aria-label={
									item._tag === "Change"
										? `${item.change.status.type} ${item.change.path}`
										: `Conflict ${item.path}`
								}
								render={
									<OperationSourceC
										projectId={projectId}
										source={operand}
										outline="outside"
										render={
											<FileRow
												item={item}
												depth={row.depth}
												pathDisplay={pathDisplay}
												inert={!navigationIndexIncludes(navigationIndex, row.path, (path) => path)}
												isSelected={isSelected}
												isChecked={checkedOperandKeys.has(operandIdentityKey(operand))}
												onSelect={() => onRowSelection(row.path)}
												canCheck={canCheck}
												checkFile={checkFile}
												projectId={projectId}
												fileParent={fileParent}
												canUncommit={canUncommit}
												uncommit={uncommit}
												selectionScope={selectionScope}
												branchNameByCommitId={(commitId) =>
													headInfoIndex?.commitContextByCommitId(commitId)?.segment.refName
														?.displayName
												}
											/>
										}
									/>
								}
							/>
						);
					})}
				</div>
			)}
		</div>
	);
};

const treeItemId = (path: string): string => `files-treeitem-${encodeURIComponent(path)}`;

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
		}),
	});
