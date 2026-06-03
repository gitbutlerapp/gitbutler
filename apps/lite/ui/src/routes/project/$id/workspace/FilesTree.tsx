import { changesInWorktreeQueryOptions, listProjectsQueryOptions } from "#ui/api/queries.ts";
import { useCommitUncommitChanges } from "#ui/api/mutations.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import {
	branchFileParent,
	changesFileParent,
	commitFileParent,
	fileOperand,
	operandEquals,
	operandIdentityKey,
	type CommitOperand,
	type Operand,
} from "#ui/operands.ts";
import {
	projectActions,
	selectProjectOutlineModeState,
	selectProjectSelectionFiles,
} from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { classes } from "#ui/components/classes.ts";
import { mergeProps, useRender } from "@base-ui/react";
import { Toolbar } from "@base-ui/react/toolbar";
import type { CommitDetails, TreeChange, TreeChanges, WorktreeChanges } from "@gitbutler/but-sdk";
import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { Array, Match } from "effect";
import { ComponentProps, createContext, FC, ReactNode, use, useEffect, useRef } from "react";
import styles from "./FilesTree.module.css";
import workspaceItemRowStyles from "./WorkspaceItemRow.module.css";
import {
	WorkspaceItemRow,
	WorkspaceItemRowEmpty,
	WorkspaceItemRowToolbar,
} from "./WorkspaceItemRow.tsx";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { getDependencyCommitIds, getHunkDependencyDiffsByPath } from "#ui/hunk.ts";
import { DependencyIndicator } from "#ui/routes/project/$id/workspace/DependencyIndicator.tsx";
import { focusSelectionScope, useNavigationIndexHotkeys } from "#ui/selection-scopes.ts";
import {
	buildNavigationIndex,
	NavigationIndex,
	navigationIndexIncludes,
} from "#ui/workspace/navigation-index.ts";
import { changesFileHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import { assert } from "#ui/assert.ts";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";

const NavigationIndexContext = createContext<NavigationIndex | null>(null);

const useNavigationIndex = (projectId: string, files: Array<Operand>) => {
	const dispatch = useAppDispatch();

	const navigationIndex = buildNavigationIndex([{ section: null, children: files }]);

	const selection = useAppSelector((state) => selectProjectSelectionFiles(state, projectId));

	// Reset selection when it's no longer part of the files list if there's a viable alternative i.e.
	// there are any files.
	//
	// React allows state updates on render, but not for external stores.
	// https://react.dev/learn/you-might-not-need-an-effect#adjusting-some-state-when-a-prop-changes
	useEffect(() => {
		if (selection && navigationIndexIncludes(navigationIndex, selection)) return;

		const next = files[0] ?? null;
		if (next === null && selection === null) return;

		dispatch(
			projectActions.selectFiles({
				projectId,
				selection: next,
			}),
		);
	}, [navigationIndex, selection, projectId, dispatch, files]);

	return navigationIndex;
};

const useFilesTreeHotkeys = ({
	navigationIndex,
	onFileSelection,
	projectId,
	ref,
}: {
	navigationIndex: NavigationIndex;
	onFileSelection: (selection: Operand) => void;
	projectId: string;
	ref: React.RefObject<HTMLElement | null>;
}) => {
	const selection = useAppSelector((state) => selectProjectSelectionFiles(state, projectId));
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));

	const dispatch = useAppDispatch();

	const selectedChangesFile =
		selection?._tag === "File" && selection.parent._tag === "Changes" ? selection : null;

	const absorbSelectedFile = () => {
		if (selectedChangesFile === null) return;

		const change = worktreeChanges?.changes.find(
			(change) => change.path === selectedChangesFile.path,
		);
		if (!change) return;

		dispatch(
			projectActions.enterAbsorbMode({
				projectId,
				source: selectedChangesFile,
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
	});
};

export const CommitFilesTree: FC<
	{
		projectId: string;
		commit: CommitOperand;
		commitDetails: CommitDetails;
		onFileSelection: (selection: Operand) => void;
	} & ComponentProps<"div">
> = ({ projectId, commit, commitDetails, onFileSelection, ...props }) => {
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

	return (
		<FilesTree
			{...props}
			projectId={projectId}
			items={[
				...conflictedPaths.map((path) =>
					conflictFileTreeItem({
						operand: fileOperand({
							parent: commitFileParent(commit),
							path,
						}),
						path,
					}),
				),
				...commitDetails.changes
					.filter((change) => !conflictedPathSet.has(change.path))
					.map((change) =>
						changeFileTreeItem({
							change,
							operand: fileOperand({
								parent: commitFileParent(commit),
								path: change.path,
							}),
						}),
					),
			]}
			onFileSelection={onFileSelection}
		/>
	);
};

export const ChangesFilesTree: FC<
	{
		projectId: string;
		worktreeChanges: WorktreeChanges;
		onFileSelection: (selection: Operand) => void;
	} & ComponentProps<"div">
> = ({ projectId, worktreeChanges, onFileSelection, ...props }) => {
	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);

	return (
		<FilesTree
			{...props}
			projectId={projectId}
			items={worktreeChanges.changes.map((change) => {
				const hunkDependencyDiffs = hunkDependencyDiffsByPath.get(change.path);
				const dependencyCommitIds = hunkDependencyDiffs
					? getDependencyCommitIds({ hunkDependencyDiffs })
					: undefined;

				return changeFileTreeItem({
					change,
					dependencyCommitIds,
					operand: fileOperand({
						parent: changesFileParent,
						path: change.path,
					}),
				});
			})}
			onFileSelection={onFileSelection}
		/>
	);
};

export const BranchFilesTree: FC<
	{
		projectId: string;
		stackId: string;
		branchRef: Array<number>;
		branchDiff: TreeChanges;
		onFileSelection: (selection: Operand) => void;
	} & ComponentProps<"div">
> = ({ projectId, stackId, branchRef, branchDiff, onFileSelection, ...props }) => (
	<FilesTree
		{...props}
		projectId={projectId}
		onFileSelection={onFileSelection}
		items={branchDiff.changes.map((change) =>
			changeFileTreeItem({
				change,
				operand: fileOperand({
					parent: branchFileParent({ stackId, branchRef }),
					path: change.path,
				}),
			}),
		)}
	/>
);

type ChangeFileTreeItem = {
	change: TreeChange;
	dependencyCommitIds?: Array.NonEmptyArray<string>;
	operand: Operand;
};

const changeFileTreeItem = ({
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
	operand: Operand;
	path: string;
};

const conflictFileTreeItem = ({ operand, path }: ConflictFileTreeItem): FileTreeItem => ({
	_tag: "Conflict",
	operand,
	path,
});

type FileTreeItem =
	| ({ _tag: "Change" } & ChangeFileTreeItem)
	| ({ _tag: "Conflict" } & ConflictFileTreeItem);

const FilesTree: FC<
	{
		projectId: string;
		items: Array<FileTreeItem>;
		onFileSelection: (selection: Operand) => void;
	} & ComponentProps<"div">
> = ({ className, items, onFileSelection, projectId, ref: refProp, ...props }) => {
	const files = items.map((item) => item.operand);

	const navigationIndex = useNavigationIndex(projectId, files);
	const selection = useAppSelector((state) => selectProjectSelectionFiles(state, projectId));

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
				className={classes(className, styles.tree)}
				ref={useMergedRefs(refProp, ref)}
			>
				<div className={workspaceItemRowStyles.section}>
					{items.length === 0 ? (
						<WorkspaceItemRowEmpty>No changes.</WorkspaceItemRowEmpty>
					) : (
						<div role="group">
							{items.map((item) => (
								<FileTreeRow
									key={operandIdentityKey(item.operand)}
									item={item}
									onFileSelection={onFileSelection}
									projectId={projectId}
								/>
							))}
						</div>
					)}
				</div>
			</div>
		</NavigationIndexContext>
	);
};

const useIsSelected = ({ projectId, operand }: { projectId: string; operand: Operand }): boolean =>
	useAppSelector((state) => {
		const selection = selectProjectSelectionFiles(state, projectId);

		return selection !== null && operandEquals(selection, operand);
	});

const treeItemId = (operand: Operand): string =>
	`files-treeitem-${encodeURIComponent(operandIdentityKey(operand))}`;

const changeLabel = (change: TreeChange): [enhancedLabel: ReactNode, rawLabel: string] => {
	const status = Match.value(change.status).pipe(
		Match.when({ type: "Addition" }, () => "A"),
		Match.when({ type: "Deletion" }, () => "D"),
		Match.when({ type: "Modification" }, () => "M"),
		Match.when({ type: "Rename" }, () => "R"),
		Match.exhaustive,
	);

	return [
		<>
			<span className={styles.fileStatusChar} data-char={status}>
				{status}
			</span>{" "}
			{change.path}
		</>,
		`${status} ${change.path}`,
	];
};

const ItemRow: FC<
	{
		onFileSelection: (selection: Operand) => void;
		projectId: string;
		operand: Operand;
	} & Omit<ComponentProps<typeof WorkspaceItemRow>, "inert" | "isSelected" | "onSelect">
> = ({ onFileSelection, projectId, operand, ...props }) => {
	const navigationIndex = assert(use(NavigationIndexContext));
	const isSelected = useIsSelected({ projectId, operand });

	return (
		<WorkspaceItemRow
			{...props}
			inert={!navigationIndexIncludes(navigationIndex, operand)}
			isSelected={isSelected}
			onSelect={() => onFileSelection(operand)}
		/>
	);
};

const TreeItem: FC<
	{
		projectId: string;
		operand: Operand;
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

const FileTreeRow: FC<{
	item: FileTreeItem;
	onFileSelection: (selection: Operand) => void;
	projectId: string;
}> = ({ item, onFileSelection, projectId }) => {
	const relativePath = item._tag === "Change" ? item.change.path : item.path;
	const [label, strLabel] =
		item._tag === "Change" ? changeLabel(item.change) : [`C ${item.path}`, `C ${item.path}`];

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const dispatch = useAppDispatch();

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);

	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");

	const commitUncommitChanges = useCommitUncommitChanges();

	const menuItems: Array<NativeMenuItem> = [
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
		...Match.value(item).pipe(
			Match.when(
				{ _tag: "Change", operand: { _tag: "File", parent: { _tag: "Commit" } } },
				({ change, operand }) => {
					const uncommit = () =>
						commitUncommitChanges.mutate({
							projectId,
							commitId: operand.parent.commitId,
							assignTo: null,
							changes: [createDiffSpec(change, [])],
							dryRun: false,
						});

					return [
						nativeMenuSeparator,
						nativeMenuItem({
							label: "Uncommit",
							enabled: !commitUncommitChanges.isPending,
							onSelect: uncommit,
						}),
					];
				},
			),
			Match.when(
				{ _tag: "Change", operand: { _tag: "File", parent: { _tag: "Changes" } } },
				({ change, operand }) => {
					const absorb = () => {
						dispatch(
							projectActions.enterAbsorbMode({
								projectId,
								source: operand,
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

					return [
						nativeMenuSeparator,
						nativeMenuItem({
							label: "Absorb",
							accelerator: toElectronAccelerator(changesFileHotkeys.absorb.hotkey),
							onSelect: absorb,
						}),
					];
				},
			),
			Match.orElse(() => []),
		),
	];

	return (
		<TreeItem
			projectId={projectId}
			operand={item.operand}
			aria-label={strLabel}
			render={
				<OperationSourceC
					projectId={projectId}
					selectionScope="files"
					source={item.operand}
					render={
						<ItemRow
							projectId={projectId}
							operand={item.operand}
							onFileSelection={onFileSelection}
						/>
					}
				/>
			}
		>
			<div
				className={workspaceItemRowStyles.itemRowLabel}
				onContextMenu={(event) => {
					void showNativeContextMenu(event, menuItems);
				}}
			>
				{label}
			</div>

			{outlineMode._tag === "Default" &&
				Match.value(item).pipe(
					Match.when(
						{ _tag: "Change", operand: { _tag: "File", parent: { _tag: "Changes" } } },
						(item) => (
							<>
								<Toolbar.Root aria-label="File actions" render={<WorkspaceItemRowToolbar />}>
									<Toolbar.Button
										aria-label="File menu"
										onClick={(event) => {
											void showNativeMenuFromTrigger(event.currentTarget, menuItems);
										}}
										className={workspaceItemRowStyles.itemRowIconButton}
									>
										<Icon name="kebab" />
									</Toolbar.Button>
								</Toolbar.Root>
								{item.dependencyCommitIds && (
									<DependencyIndicator
										projectId={projectId}
										commitIds={item.dependencyCommitIds}
										className={workspaceItemRowStyles.itemRowIconButton}
									>
										<Icon name="link" />
									</DependencyIndicator>
								)}
							</>
						),
					),
					Match.orElse(() => null),
				)}
		</TreeItem>
	);
};
