import workspaceItemRowStyles from "./WorkspaceItemRow.module.css";
import { changesInWorktreeQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { showNativeContextMenu, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import { uncommittedChangesFileParent, fileOperand, FileParent } from "#ui/operands.ts";
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
import { useQuery } from "@tanstack/react-query";
import { identity, Match } from "effect";
import { ComponentProps, createContext, FC, use, useRef } from "react";
import styles from "./FilesTree.module.css";
import {
	WorkspaceItemRow,
	WorkspaceItemRowLabel,
	WorkspaceItemRowLabelContainer,
	WorkspaceItemRowToolbar,
} from "./WorkspaceItemRow.tsx";
import { getWorkspaceItemRowButtonClassName } from "./WorkspaceItemRow-utils.ts";
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
import type { FileTreeItem } from "./file-tree.ts";

const NavigationIndexContext = createContext<NavigationIndex<string> | null>(null);

const useFilesTreeHotkeys = ({
	navigationIndex,
	onFileSelection,
	projectId,
	ref,
	fileParent,
}: {
	navigationIndex: NavigationIndex<string>;
	onFileSelection: (selection: string) => void;
	projectId: string;
	ref: React.RefObject<HTMLElement | null>;
	fileParent: FileParent;
}) => {
	const selection = useFilesSelection(projectId, navigationIndex);
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));

	const dispatch = useAppDispatch();

	const selectedChangesFile = fileParent._tag === "UncommittedChanges" ? selection : null;

	const absorbSelectedFile = () => {
		if (selectedChangesFile === null) return;

		const change = worktreeChanges?.changes.find((change) => change.path === selectedChangesFile);
		if (!change) return;

		dispatch(
			projectActions.enterAbsorbMode({
				projectId,
				source: fileOperand({ parent: uncommittedChangesFileParent, path: selectedChangesFile }),
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
		group: "File",
		selectionScope: "files",
		select: onFileSelection,
		selection,
		ref,
		getKey: identity,
		operationSourceForItem: (path) => fileOperand({ parent: fileParent, path }),
	});
};

export const FilesTree: FC<
	{
		projectId: string;
		items: Array<FileTreeItem>;
		onFileSelection: (selection: string) => void;
		navigationIndex: NavigationIndex<string>;
		fileParent: FileParent;
	} & ComponentProps<"div">
> = ({
	items,
	onFileSelection,
	projectId,
	navigationIndex,
	fileParent,
	ref: refProp,
	...props
}) => {
	const selection = useFilesSelection(projectId, navigationIndex);
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});

	const ref = useRef<HTMLDivElement>(null);

	useFilesTreeHotkeys({
		navigationIndex,
		onFileSelection,
		projectId,
		ref,
		fileParent,
	});

	return (
		<NavigationIndexContext value={navigationIndex}>
			<div
				{...props}
				tabIndex={0}
				role="tree"
				aria-activedescendant={selection !== null ? treeItemId(selection) : undefined}
				className={classes(props.className, styles.tree)}
				ref={useMergedRefs(refProp, ref)}
			>
				<div className={styles.section}>
					{items.length === 0 ? (
						<WorkspaceItemRow interactive={false}>
							<WorkspaceItemRowLabelContainer>
								<WorkspaceItemRowLabel className={workspaceItemRowStyles.fadedText}>
									No changes.
								</WorkspaceItemRowLabel>
							</WorkspaceItemRowLabelContainer>
						</WorkspaceItemRow>
					) : (
						// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- New lint violation.
						<div role="group">
							{items.map((item) => (
								<TreeItem
									key={item.path}
									projectId={projectId}
									path={item.path}
									aria-label={
										item._tag === "Change"
											? `${item.change.status.type} ${item.change.path}`
											: `Conflict ${item.path}`
									}
									render={
										<OperationSourceC
											projectId={projectId}
											source={fileOperand({ parent: fileParent, path: item.path })}
											render={
												<FileRow
													item={item}
													path={item.path}
													onFileSelection={onFileSelection}
													projectId={projectId}
													fileParent={fileParent}
													branchNameByCommitId={(cid) =>
														headInfoIndex?.commitContextById(cid)?.segment.refName?.displayName
													}
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

const useIsSelected = ({ projectId, path }: { projectId: string; path: string }): boolean => {
	const navigationIndex = assert(use(NavigationIndexContext));
	return useAppSelector((state) => {
		const selectionState = selectProjectSelectionFiles(state, projectId);
		const selection = resolveNavigationIndexSelection(navigationIndex, selectionState, identity);

		return selection !== null && selection === path;
	});
};

const treeItemId = (path: string): string => `files-treeitem-${encodeURIComponent(path)}`;

const ItemRow: FC<
	{
		onFileSelection: (selection: string) => void;
		projectId: string;
		path: string;
	} & Omit<ComponentProps<typeof WorkspaceItemRow>, "inert" | "isSelected" | "onSelect">
> = ({ onFileSelection, projectId, path, ...props }) => {
	const navigationIndex = assert(use(NavigationIndexContext));
	const isSelected = useIsSelected({ projectId, path });

	return (
		<WorkspaceItemRow
			{...props}
			inert={!navigationIndexIncludes(navigationIndex, path, identity)}
			isSelected={isSelected}
			onSelect={() => onFileSelection(path)}
		/>
	);
};

const TreeItem: FC<
	{
		projectId: string;
		path: string;
	} & useRender.ComponentProps<"div">
> = ({ projectId, path, render, ...props }) => {
	const isSelected = useIsSelected({ projectId, path });

	return useRender({
		render,
		defaultTagName: "div",
		props: mergeProps<"div">(props, {
			id: treeItemId(path),
			role: "treeitem",
			"aria-selected": isSelected,
		}),
	});
};

const FileRow: FC<
	{
		item: FileTreeItem;
		projectId: string;
		fileParent: FileParent;
		branchNameByCommitId?: (commitId: string) => string | undefined;
	} & Omit<ComponentProps<typeof ItemRow>, "projectId">
> = ({ item, projectId, fileParent, branchNameByCommitId, id, ...restProps }) => {
	const relativePath = item._tag === "Change" ? item.change.path : item.path;

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const menuItems = useFileMenuItems({
		projectId,
		operand: { parent: fileParent, path: relativePath },
		path: relativePath,
		change: item._tag === "Change" ? item.change : undefined,
	});

	const hasCheckedCommits = useAppSelector((state) =>
		selectProjectHasCheckedCommits(state, projectId),
	);

	const lastSepIdx = relativePath.lastIndexOf("/");
	const mpathInit = lastSepIdx !== -1 ? relativePath.slice(0, lastSepIdx + 1) : null;
	const pathLast = lastSepIdx !== -1 ? relativePath.slice(lastSepIdx + 1) : relativePath;

	return (
		<Tooltip.Root disableHoverablePopup>
			<Tooltip.Trigger
				// We pass the ID here instead of including it with the other props as a
				// workaround for Base UI issue:
				// https://github.com/mui/base-ui/issues/5108
				id={id}
				render={
					<ItemRow
						{...restProps}
						projectId={projectId}
						path={item.path}
						className={classes(restProps.className, styles.fileRow)}
						onContextMenu={(event) => {
							void showNativeContextMenu(event, menuItems);
						}}
					/>
				}
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

				<WorkspaceItemRowLabelContainer>
					<WorkspaceItemRowLabel singleLine className={styles.filePath}>
						{mpathInit}
						<span className={styles.pathLast}>{pathLast}</span>
						{item._tag === "Conflict" && " ⚠️"}
					</WorkspaceItemRowLabel>
				</WorkspaceItemRowLabelContainer>

				{outlineMode._tag === "Default" && (
					<Toolbar.Root aria-label="File actions" render={<WorkspaceItemRowToolbar />}>
						<Toolbar.Button
							aria-label="File menu"
							onClick={(event) => {
								void showNativeMenuFromTrigger(event.currentTarget, menuItems);
							}}
							className={getWorkspaceItemRowButtonClassName({ iconOnly: true })}
						>
							<Icon name="kebab" />
						</Toolbar.Button>

						{item._tag === "Change" &&
							fileParent._tag === "UncommittedChanges" &&
							item.dependencyCommitIds && (
								<Toolbar.Button
									render={
										<DependencyIndicator
											projectId={projectId}
											commitIds={item.dependencyCommitIds}
											branchNameByCommitId={branchNameByCommitId}
											className={getWorkspaceItemRowButtonClassName({ iconOnly: true })}
										/>
									}
								>
									<Icon name="link" />
								</Toolbar.Button>
							)}
					</Toolbar.Root>
				)}

				{item._tag === "Change" && (
					<Tooltip.Root disableHoverablePopup>
						<Tooltip.Trigger
							className={styles.fileStatusBadge}
							aria-label={item.change.status.type}
							data-status-type={item.change.status.type}
							// By default it's a button, but we don't want this to be
							// interactive.
							render={<span />}
						>
							{Match.value(item.change.status).pipe(
								Match.when({ type: "Addition" }, () => "A"),
								Match.when({ type: "Deletion" }, () => "D"),
								Match.when({ type: "Modification" }, () => "M"),
								Match.when({ type: "Rename" }, () => "R"),
								Match.exhaustive,
							)}
						</Tooltip.Trigger>
						<Tooltip.Portal>
							<Tooltip.Positioner sideOffset={4}>
								<Tooltip.Popup render={<TooltipPopup />}>{item.change.status.type}</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
				)}
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>{relativePath}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
