import { changesInWorktreeQueryOptions } from "#ui/api/queries.ts";
import { showNativeContextMenu, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import { fileOperand, operandIdentityKey, type FileOperand } from "#ui/operands.ts";
import {
	projectActions,
	selectProjectHasCheckedCommits,
	selectProjectOutlineModeState,
	selectProjectSelectionFiles,
} from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { Checkbox } from "#ui/components/Checkbox.tsx";
import { classes } from "#ui/components/classes.ts";
import { mergeProps, Tooltip, useRender } from "@base-ui/react";
import { Toolbar } from "@base-ui/react/toolbar";
import type { TreeChange, TreeStatus } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import { Array, Match } from "effect";
import { ComponentProps, createContext, FC, use, useRef } from "react";
import styles from "./FilesTree.module.css";
import {
	WorkspaceItemRow,
	WorkspaceItemRowLabel,
	WorkspaceItemRowToolbar,
	getWorkspaceItemRowButtonClassName,
} from "./WorkspaceItemRow.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { DependencyIndicator } from "#ui/routes/project/$id/workspace/DependencyIndicator.tsx";
import {
	focusSelectionScope,
	resolveNavigationIndexSelection,
	useFilesSelection,
	useNavigationIndexHotkeys,
} from "#ui/selection-scopes.ts";
import { navigationIndexIncludes, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { changesFileHotkeys } from "#ui/hotkeys.ts";
import { assert } from "#ui/assert.ts";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { useFileMenuItems } from "#ui/routes/project/$id/workspace/useFileMenuItems.ts";

const fileOperandIdentityKey = (operand: FileOperand): string =>
	operandIdentityKey(fileOperand(operand));

const NavigationIndexContext = createContext<NavigationIndex<FileOperand> | null>(null);

const useFilesTreeHotkeys = ({
	navigationIndex,
	onFileSelection,
	projectId,
	ref,
}: {
	navigationIndex: NavigationIndex<FileOperand>;
	onFileSelection: (selection: FileOperand) => void;
	projectId: string;
	ref: React.RefObject<HTMLElement | null>;
}) => {
	const selection = useFilesSelection(projectId, navigationIndex);
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));

	const dispatch = useAppDispatch();

	const selectedChangesFile = selection?.parent._tag === "Changes" ? selection : null;

	const absorbSelectedFile = () => {
		if (selectedChangesFile === null) return;

		const change = worktreeChanges?.changes.find(
			(change) => change.path === selectedChangesFile.path,
		);
		if (!change) return;

		dispatch(
			projectActions.enterAbsorbMode({
				projectId,
				source: fileOperand(selectedChangesFile),
				sourceTarget: {
					type: "treeChanges",
					subject: {
						changes: [change],
						assignedStackId: null,
					},
				},
			}),
		);
		focusSelectionScope("outline");
	};

	useHotkeys([
		{
			hotkey: changesFileHotkeys.absorb.hotkey,
			callback: absorbSelectedFile,
			options: {
				conflictBehavior: "allow",
				enabled: selectedChangesFile !== null && outlineMode._tag === "Default",
				target: ref,
				meta: changesFileHotkeys.absorb.meta,
			},
		},
	]);

	useNavigationIndexHotkeys({
		navigationIndex,
		projectId,
		group: "Files",
		selectionScope: "files",
		select: onFileSelection,
		selection,
		ref,
		getKey: fileOperandIdentityKey,
		operationSourceForItem: fileOperand,
	});
};

type ChangeFileTreeItem = {
	change: TreeChange;
	dependencyCommitIds?: Array.NonEmptyArray<string>;
	operand: FileOperand;
};

export const changeFileTreeItem = ({
	change,
	dependencyCommitIds,
	operand,
}: ChangeFileTreeItem): FileTreeItem => ({
	_tag: "Change",
	change,
	dependencyCommitIds,
	operand,
});

type ConflictFileTreeItem = {
	operand: FileOperand;
	path: string;
};

export const conflictFileTreeItem = ({ operand, path }: ConflictFileTreeItem): FileTreeItem => ({
	_tag: "Conflict",
	operand,
	path,
});

export type FileTreeItem =
	| ({ _tag: "Change" } & ChangeFileTreeItem)
	| ({ _tag: "Conflict" } & ConflictFileTreeItem);

export const FilesTree: FC<
	{
		projectId: string;
		items: Array<FileTreeItem>;
		onFileSelection: (selection: FileOperand) => void;
		navigationIndex: NavigationIndex<FileOperand>;
	} & ComponentProps<"div">
> = ({ items, onFileSelection, projectId, navigationIndex, ref: refProp, ...props }) => {
	const selection = useFilesSelection(projectId, navigationIndex);

	const ref = useRef<HTMLDivElement>(null);

	useFilesTreeHotkeys({
		navigationIndex,
		onFileSelection,
		projectId,
		ref,
	});

	return (
		<NavigationIndexContext value={navigationIndex}>
			<div
				{...props}
				tabIndex={0}
				role="tree"
				aria-activedescendant={selection ? treeItemId(selection) : undefined}
				className={classes(props.className, styles.tree)}
				ref={useMergedRefs(refProp, ref)}
			>
				<div className={styles.section}>
					{items.length === 0 ? (
						<WorkspaceItemRow interactive={false}>
							<WorkspaceItemRowLabel empty>No changes.</WorkspaceItemRowLabel>
						</WorkspaceItemRow>
					) : (
						<div role="group">
							{items.map((item) => (
								<TreeItem
									key={fileOperandIdentityKey(item.operand)}
									projectId={projectId}
									operand={item.operand}
									aria-label={
										item._tag === "Change"
											? `${statusLabel(item.change.status)} ${item.change.path}`
											: `C ${item.path}`
									}
									render={
										<OperationSourceC
											projectId={projectId}
											source={fileOperand(item.operand)}
											onDragStart={() => onFileSelection(item.operand)}
											render={
												<FileRow
													item={item}
													onFileSelection={onFileSelection}
													projectId={projectId}
												/>
											}
										/>
									}
								/>
							))}
						</div>
					)}
				</div>
			</div>
		</NavigationIndexContext>
	);
};

const useIsSelected = ({
	projectId,
	operand,
}: {
	projectId: string;
	operand: FileOperand;
}): boolean => {
	const navigationIndex = assert(use(NavigationIndexContext));
	return useAppSelector((state) => {
		const selectionState = selectProjectSelectionFiles(state, projectId);
		const selection = resolveNavigationIndexSelection(
			navigationIndex,
			selectionState,
			fileOperandIdentityKey,
		);

		return (
			selection !== null && fileOperandIdentityKey(selection) === fileOperandIdentityKey(operand)
		);
	});
};

const treeItemId = (operand: FileOperand): string =>
	`files-treeitem-${encodeURIComponent(fileOperandIdentityKey(operand))}`;

const ItemRow: FC<
	{
		onFileSelection: (selection: FileOperand) => void;
		projectId: string;
		operand: FileOperand;
	} & Omit<ComponentProps<typeof WorkspaceItemRow>, "inert" | "isSelected" | "onSelect">
> = ({ onFileSelection, projectId, operand, ...props }) => {
	const navigationIndex = assert(use(NavigationIndexContext));
	const isSelected = useIsSelected({ projectId, operand });

	return (
		<WorkspaceItemRow
			{...props}
			inert={!navigationIndexIncludes(navigationIndex, operand, fileOperandIdentityKey)}
			isSelected={isSelected}
			onSelect={() => onFileSelection(operand)}
		/>
	);
};

const TreeItem: FC<
	{
		projectId: string;
		operand: FileOperand;
	} & useRender.ComponentProps<"div">
> = ({ projectId, operand, render, ...props }) => {
	const isSelected = useIsSelected({ projectId, operand });

	return useRender({
		render,
		defaultTagName: "div",
		props: mergeProps<"div">(props, {
			id: treeItemId(operand),
			role: "treeitem",
			"aria-selected": isSelected,
		}),
	});
};

const statusLabel = (status: TreeStatus): string =>
	Match.value(status).pipe(
		Match.when({ type: "Addition" }, () => "A"),
		Match.when({ type: "Deletion" }, () => "D"),
		Match.when({ type: "Modification" }, () => "M"),
		Match.when({ type: "Rename" }, () => "R"),
		Match.exhaustive,
	);

const FileRow: FC<
	{
		item: FileTreeItem;
		onFileSelection: (selection: FileOperand) => void;
		projectId: string;
	} & Omit<ComponentProps<typeof ItemRow>, "onFileSelection" | "projectId" | "operand">
> = ({ item, onFileSelection, projectId, ...restProps }) => {
	const relativePath = item._tag === "Change" ? item.change.path : item.path;

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const menuItems = useFileMenuItems({
		projectId,
		operand: item.operand,
		path: relativePath,
		change: item._tag === "Change" ? item.change : undefined,
	});

	const hasCheckedCommits = useAppSelector((state) =>
		selectProjectHasCheckedCommits(state, projectId),
	);

	return (
		<ItemRow
			{...restProps}
			projectId={projectId}
			operand={item.operand}
			onFileSelection={onFileSelection}
			className={classes(restProps.className, styles.fileRow)}
			onContextMenu={(event) => {
				void showNativeContextMenu(event, menuItems);
			}}
		>
			<div className={styles.fileIconWithCheckbox}>
				<Icon name="file" />
				<Tooltip.Root
					// This gets in the way when the user tries to move their hover to a
					// sibling row.
					disableHoverablePopup
				>
					<Checkbox
						disabled={hasCheckedCommits || outlineMode._tag !== "Default"}
						aria-label={`Check file ${relativePath}`}
						className={styles.fileCheckbox}
						nativeButton
						render={<Tooltip.Trigger />}
					/>
					<Tooltip.Portal>
						<Tooltip.Positioner sideOffset={4}>
							<Tooltip.Popup render={<TooltipPopup />}>Check file</Tooltip.Popup>
						</Tooltip.Positioner>
					</Tooltip.Portal>
				</Tooltip.Root>
			</div>
			<WorkspaceItemRowLabel>
				{item._tag === "Change" ? item.change.path : item.path}
			</WorkspaceItemRowLabel>

			{outlineMode._tag === "Default" && (
				<Toolbar.Root aria-label="File actions" render={<WorkspaceItemRowToolbar />}>
					{Match.value(item).pipe(
						Match.when(
							{ _tag: "Change", operand: { parent: { _tag: "Changes" } } },
							(item) =>
								item.dependencyCommitIds && (
									<Toolbar.Button
										render={
											<DependencyIndicator
												projectId={projectId}
												commitIds={item.dependencyCommitIds}
												className={getWorkspaceItemRowButtonClassName({ iconOnly: true })}
											/>
										}
									>
										<Icon name="link" />
									</Toolbar.Button>
								),
						),
						Match.orElse(() => null),
					)}
					<Toolbar.Button
						aria-label="File menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getWorkspaceItemRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</Toolbar.Button>
				</Toolbar.Root>
			)}

			{item._tag === "Change" ? (
				<Icon
					name="file"
					className={styles.fileStatusIcon}
					data-char={statusLabel(item.change.status)}
				/>
			) : (
				"C"
			)}
		</ItemRow>
	);
};
