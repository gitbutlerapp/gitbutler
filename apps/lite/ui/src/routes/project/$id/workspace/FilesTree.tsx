import { changesInWorktreeQueryOptions, listProjectsQueryOptions } from "#ui/api/queries.ts";
import {
	useCommitDiscardChanges,
	useCommitUncommitChanges,
	useDiscardWorktreeChanges,
} from "#ui/api/mutations.ts";
import {
	nativeMenuItem,
	nativeMenuItemsFromGroups,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import { changesFileParent, fileOperand, FileParent, type FileOperand } from "#ui/operands.ts";
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
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { Array, identity, Match } from "effect";
import { ComponentProps, createContext, FC, use, useRef } from "react";
import styles from "./FilesTree.module.css";
import {
	WorkspaceItemRow,
	WorkspaceItemRowEmpty,
	WorkspaceItemRowIconButton,
	WorkspaceItemRowLabel,
	WorkspaceItemRowToolbar,
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
import { changesFileHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import { assert } from "#ui/assert.ts";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { NonEmptyArray } from "effect/Array";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";

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

	const selectedChangesFile = fileParent._tag === "Changes" ? selection : null;

	const absorbSelectedFile = () => {
		if (selectedChangesFile === null) return;

		const change = worktreeChanges?.changes.find((change) => change.path === selectedChangesFile);
		if (!change) return;

		dispatch(
			projectActions.enterAbsorbMode({
				projectId,
				source: fileOperand({ parent: changesFileParent, path: selectedChangesFile }),
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
		getKey: identity,
		operationSourceForItem: (path) => fileOperand({ parent: fileParent, path }),
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
						<WorkspaceItemRowEmpty>No changes.</WorkspaceItemRowEmpty>
					) : (
						<div role="group">
							{items.map((item) => (
								<TreeItem
									key={item.operand.path}
									projectId={projectId}
									path={item.operand.path}
									aria-label={
										item._tag === "Change"
											? `${statusLabel(item.change.status)} ${item.change.path}`
											: `C ${item.path}`
									}
									render={
										<OperationSourceC
											projectId={projectId}
											source={fileOperand(item.operand)}
											onDragStart={() => onFileSelection(item.operand.path)}
											render={
												<FileRow
													item={item}
													path={item.operand.path}
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
		projectId: string;
	} & Omit<ComponentProps<typeof ItemRow>, "projectId" | "operand">
> = ({ item, projectId, ...restProps }) => {
	const relativePath = item._tag === "Change" ? item.change.path : item.path;

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const dispatch = useAppDispatch();

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);

	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");

	const commitUncommitChanges = useCommitUncommitChanges();
	const commitDiscardChanges = useCommitDiscardChanges();
	const discardWorktreeChanges = useDiscardWorktreeChanges();

	const menuItemGroups: Array<NonEmptyArray<NativeMenuItem>> = [
		[
			nativeMenuItem({
				label: "Copy Path",
				submenu: [
					nativeMenuItem({
						label: "Absolute Path",
						onSelect: async () => {
							const absolutePath = await window.lite.pathJoin(selectedProject.path, relativePath);
							await window.lite.clipboardWriteText(absolutePath);
						},
					}),
					nativeMenuItem({
						label: "Relative Path",
						onSelect: () => window.lite.clipboardWriteText(relativePath),
					}),
				],
			}),
		],
		...Match.value(item).pipe(
			Match.withReturnType<Array<NonEmptyArray<NativeMenuItem>>>(),
			Match.when(
				{ _tag: "Change", operand: { parent: { _tag: "Commit" } } },
				({ change, operand }) => {
					const uncommit = () =>
						commitUncommitChanges.mutate({
							projectId,
							commitId: operand.parent.commitId,
							assignTo: null,
							changes: [createDiffSpec(change, [])],
							dryRun: false,
						});
					const discard = () =>
						commitDiscardChanges.mutate({
							projectId,
							commitId: operand.parent.commitId,
							changes: [createDiffSpec(change, [])],
							dryRun: false,
						});

					return [
						[
							nativeMenuItem({
								label: "Uncommit",
								enabled: !commitUncommitChanges.isPending,
								onSelect: uncommit,
							}),
							nativeMenuItem({
								label: "Discard Changes",
								enabled: !commitDiscardChanges.isPending,
								onSelect: discard,
							}),
						],
					];
				},
			),
			Match.when(
				{ _tag: "Change", operand: { parent: { _tag: "Changes" } } },
				({ change, operand }) => {
					const absorb = () => {
						dispatch(
							projectActions.enterAbsorbMode({
								projectId,
								source: fileOperand(operand),
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
					const discard = () =>
						discardWorktreeChanges.mutate({
							projectId,
							changes: [createDiffSpec(change, [])],
						});

					return [
						[
							nativeMenuItem({
								label: "Absorb",
								accelerator: toElectronAccelerator(changesFileHotkeys.absorb.hotkey),
								onSelect: absorb,
							}),
							nativeMenuItem({
								label: "Discard Changes",
								enabled: !discardWorktreeChanges.isPending,
								onSelect: discard,
							}),
						],
					];
				},
			),
			Match.orElse(() => []),
		),
	];
	const menuItems = nativeMenuItemsFromGroups(menuItemGroups);

	const hasCheckedCommits = useAppSelector((state) =>
		selectProjectHasCheckedCommits(state, projectId),
	);

	return (
		<ItemRow
			{...restProps}
			projectId={projectId}
			path={item.operand.path}
			className={classes(restProps.className, styles.fileRow)}
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
			<WorkspaceItemRowLabel
				onContextMenu={(event) => {
					void showNativeContextMenu(event, menuItems);
				}}
			>
				{item._tag === "Change" ? item.change.path : item.path}
			</WorkspaceItemRowLabel>

			{outlineMode._tag === "Default" && (
				<WorkspaceItemRowToolbar forceVisible>
					<Toolbar.Root aria-label="File actions" render={<WorkspaceItemRowToolbar />}>
						{Match.value(item).pipe(
							Match.when(
								{ _tag: "Change", operand: { parent: { _tag: "Changes" } } },
								(item) =>
									item.dependencyCommitIds && (
										<Toolbar.Button
											render={
												<WorkspaceItemRowIconButton
													render={
														<DependencyIndicator
															projectId={projectId}
															commitIds={item.dependencyCommitIds}
														/>
													}
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
							render={<WorkspaceItemRowIconButton />}
						>
							<Icon name="kebab" />
						</Toolbar.Button>
					</Toolbar.Root>

					{item._tag === "Change" ? (
						<Icon
							name="file"
							className={styles.fileStatusIcon}
							data-char={statusLabel(item.change.status)}
						/>
					) : (
						"C"
					)}
				</WorkspaceItemRowToolbar>
			)}
		</ItemRow>
	);
};
