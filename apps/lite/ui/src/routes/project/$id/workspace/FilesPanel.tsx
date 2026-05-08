import { useNavigationIndexHotkeys } from "#ui/panels.ts";
import {
	absorptionPlanQueryOptions,
	branchDiffQueryOptions,
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
} from "#ui/api/queries.ts";
import {
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import {
	branchFileParent,
	branchOperand,
	changesFileParent,
	changesSectionOperand,
	commitFileParent,
	commitOperand,
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
	selectProjectSelectionOutline,
} from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { classes } from "#ui/ui/classes.ts";
import { DependencyIcon, MenuTriggerIcon } from "#ui/ui/icons.tsx";
import { mergeProps, useRender } from "@base-ui/react";
import { Toolbar } from "@base-ui/react/toolbar";
import { AbsorptionTarget, TreeChange } from "@gitbutler/but-sdk";
import { useQueryClient, useSuspenseQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Array, Match } from "effect";
import { ComponentProps, FC, Suspense, useEffect } from "react";
import { Panel, PanelProps } from "react-resizable-panels";
import styles from "./FilesPanel.module.css";
import workspaceItemRowStyles from "./WorkspaceItemRow.module.css";
import { WorkspaceItemRow, WorkspaceItemRowToolbar } from "./WorkspaceItemRow.tsx";
import { decodeRefName } from "#ui/api/ref-name.ts";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { getDependencyCommitIds, getHunkDependencyDiffsByPath } from "#ui/hunk.ts";
import { DependencyIndicatorButton } from "#ui/routes/project/$id/workspace/DependencyIndicatorButton.tsx";
import { useFocusedProjectPanel } from "#ui/panels.ts";
import { useHotkey } from "@tanstack/react-hotkeys";
import {
	buildNavigationIndex,
	NavigationIndex,
	navigationIndexIncludes,
} from "#ui/workspace/navigation-index.ts";
import { filterNavigationIndexForOutlineMode } from "#ui/outline/mode.ts";

const useNavigationIndex = (projectId: string, parent: Operand, files: Array<Operand>) => {
	const dispatch = useAppDispatch();

	const navigationIndexUnfiltered = buildNavigationIndex([{ section: parent, children: files }]);

	const selection = useAppSelector((state) => selectProjectSelectionFiles(state, projectId));

	// Reset selection when it's no longer part of the workspace.
	//
	// React allows state updates on render, but not for external stores.
	// https://react.dev/learn/you-might-not-need-an-effect#adjusting-some-state-when-a-prop-changes
	useEffect(() => {
		if (!navigationIndexIncludes(navigationIndexUnfiltered, selection))
			dispatch(
				projectActions.selectFiles({
					projectId,
					selection: parent,
				}),
			);
	}, [navigationIndexUnfiltered, selection, projectId, dispatch, parent]);

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const navigationIndex = filterNavigationIndexForOutlineMode({
		navigationIndex: navigationIndexUnfiltered,
		selection,
		outlineMode,
	});

	const focusedPanel = useFocusedProjectPanel(projectId);

	const select = (newItem: Operand) =>
		dispatch(projectActions.selectFiles({ projectId, selection: newItem }));

	useNavigationIndexHotkeys({
		focusedPanel,
		navigationIndex,
		projectId,
		group: "Files",
		panel: "files",
		select,
		selection,
	});

	return navigationIndex;
};

const CommitFilesTreePanel: FC<{ projectId: string; commit: CommitOperand } & PanelProps> = ({
	projectId,
	commit,
	...panelProps
}) => {
	const { data } = useSuspenseQuery(
		commitDetailsWithLineStatsQueryOptions({ projectId, commitId: commit.commitId }),
	);

	const parent = commitOperand(commit);

	const conflictedPaths = data.conflictEntries
		? globalThis.Array.from(
				new Set([
					...data.conflictEntries.ancestorEntries,
					...data.conflictEntries.ourEntries,
					...data.conflictEntries.theirEntries,
				]),
			).toSorted((a, b) => a.localeCompare(b))
		: [];

	const files = [
		...conflictedPaths,
		...data.changes.filter((x) => !conflictedPaths.includes(x.path)).map((x) => x.path),
	].map((path) =>
		fileOperand({
			parent: commitFileParent({ stackId: commit.stackId, commitId: commit.commitId }),
			path,
		}),
	);

	const navigationIndex = useNavigationIndex(projectId, parent, files);

	return (
		<FilesTreePanel {...panelProps} navigationIndex={navigationIndex}>
			{(() => {
				if (conflictedPaths.length === 0 && data.changes.length === 0)
					return <div className={workspaceItemRowStyles.itemRowEmpty}>No file changes.</div>;

				return (
					<div role="group">
						{conflictedPaths.length > 0 &&
							conflictedPaths.map((path) => (
								<ConflictedFileRow
									operand={fileOperand({
										parent: commitFileParent(commit),
										path,
									})}
									key={path}
									path={path}
									projectId={projectId}
									navigationIndex={navigationIndex}
								/>
							))}

						{data.changes.length > 0 &&
							data.changes.map((change) => (
								<TreeChangeRow
									operand={fileOperand({
										parent: commitFileParent(commit),
										path: change.path,
									})}
									key={change.path}
									change={change}
									projectId={projectId}
									navigationIndex={navigationIndex}
								/>
							))}
					</div>
				);
			})()}
		</FilesTreePanel>
	);
};

const ChangesFilesTreePanel: FC<
	{
		projectId: string;
	} & PanelProps
> = ({ projectId, ...panelProps }) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const parent = changesSectionOperand;

	const files = worktreeChanges.changes.map((change) =>
		fileOperand({ parent: changesFileParent, path: change.path }),
	);

	const navigationIndex = useNavigationIndex(projectId, parent, files);

	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);

	return (
		<FilesTreePanel {...panelProps} navigationIndex={navigationIndex}>
			{worktreeChanges.changes.length === 0 ? (
				<div className={workspaceItemRowStyles.itemRowEmpty}>No changes.</div>
			) : (
				<div role="group">
					{worktreeChanges.changes.map((change) => {
						const hunkDependencyDiffs = hunkDependencyDiffsByPath.get(change.path);
						const dependencyCommitIds = hunkDependencyDiffs
							? getDependencyCommitIds({ hunkDependencyDiffs })
							: undefined;

						return (
							<ChangesFileRow
								key={change.path}
								change={change}
								dependencyCommitIds={dependencyCommitIds}
								projectId={projectId}
								navigationIndex={navigationIndex}
							/>
						);
					})}
				</div>
			)}
		</FilesTreePanel>
	);
};

const BranchFilesTreePanel: FC<
	{
		projectId: string;
		stackId: string;
		branchRef: Array<number>;
	} & PanelProps
> = ({ projectId, stackId, branchRef, ...panelProps }) => {
	const decodedBranchRef = decodeRefName(branchRef);
	const { data: branchDiff } = useSuspenseQuery(
		branchDiffQueryOptions({ projectId, branch: decodedBranchRef }),
	);

	const parent = branchOperand({ stackId, branchRef });

	const files = branchDiff.changes.map((change) =>
		fileOperand({
			parent: branchFileParent({ stackId, branchRef }),
			path: change.path,
		}),
	);

	const navigationIndex = useNavigationIndex(projectId, parent, files);

	return (
		<FilesTreePanel {...panelProps} navigationIndex={navigationIndex}>
			{branchDiff.changes.length === 0 ? (
				<div className={workspaceItemRowStyles.itemRowEmpty}>No changes.</div>
			) : (
				<div role="group">
					{branchDiff.changes.map((change) => (
						<TreeChangeRow
							operand={fileOperand({
								parent: branchFileParent({ stackId, branchRef }),
								path: change.path,
							})}
							key={change.path}
							change={change}
							projectId={projectId}
							navigationIndex={navigationIndex}
						/>
					))}
				</div>
			)}
		</FilesTreePanel>
	);
};

export const FilesPanel: FC<{} & PanelProps> = ({ ...panelProps }) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const outlineSelection = useAppSelector((state) =>
		selectProjectSelectionOutline(state, projectId),
	);

	return (
		<Suspense fallback={<Panel {...panelProps}>Loading files…</Panel>}>
			{Match.value(outlineSelection).pipe(
				Match.tag("Commit", (commit) => (
					<CommitFilesTreePanel {...panelProps} projectId={projectId} commit={commit} />
				)),
				Match.tag("ChangesSection", () => (
					<ChangesFilesTreePanel {...panelProps} projectId={projectId} />
				)),
				Match.tag("Branch", ({ stackId, branchRef }) => (
					<BranchFilesTreePanel
						{...panelProps}
						projectId={projectId}
						stackId={stackId}
						branchRef={branchRef}
					/>
				)),
				Match.orElse(() => <Panel {...panelProps} />),
			)}
		</Suspense>
	);
};

const FilesTreePanel: FC<{ navigationIndex: NavigationIndex } & PanelProps> = ({
	className,
	children,
	navigationIndex,
	...panelProps
}) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const outlineSelection = useAppSelector((state) =>
		selectProjectSelectionOutline(state, projectId),
	);
	const selection = useAppSelector((state) => selectProjectSelectionFiles(state, projectId));

	return (
		<Panel
			{...panelProps}
			tabIndex={0}
			role="tree"
			aria-activedescendant={treeItemId(selection)}
			className={classes(className, styles.tree)}
		>
			<TreeItem
				projectId={projectId}
				operand={outlineSelection}
				label="All changes"
				expanded
				className={workspaceItemRowStyles.section}
				render={<OperationSourceC projectId={projectId} source={outlineSelection} />}
			>
				<ItemRow projectId={projectId} operand={outlineSelection} navigationIndex={navigationIndex}>
					<div
						className={classes(
							workspaceItemRowStyles.itemRowLabel,
							workspaceItemRowStyles.sectionLabel,
						)}
					>
						All changes
					</div>
				</ItemRow>

				{children}
			</TreeItem>
		</Panel>
	);
};

const useIsSelected = ({ projectId, operand }: { projectId: string; operand: Operand }): boolean =>
	useAppSelector((state) => {
		const selection = selectProjectSelectionFiles(state, projectId);

		return operandEquals(selection, operand);
	});

const treeItemId = (operand: Operand): string =>
	`files-treeitem-${encodeURIComponent(operandIdentityKey(operand))}`;

const changeLabel = (change: TreeChange) => {
	const status = Match.value(change.status).pipe(
		Match.when({ type: "Addition" }, () => "A"),
		Match.when({ type: "Deletion" }, () => "D"),
		Match.when({ type: "Modification" }, () => "M"),
		Match.when({ type: "Rename" }, () => "R"),
		Match.exhaustive,
	);

	return `${status} ${change.path}`;
};

const ItemRow: FC<
	{
		projectId: string;
		operand: Operand;
		navigationIndex: NavigationIndex;
	} & Omit<ComponentProps<typeof WorkspaceItemRow>, "inert" | "isSelected" | "onSelect">
> = ({ projectId, operand, navigationIndex, ...props }) => {
	const dispatch = useAppDispatch();
	const isSelected = useIsSelected({ projectId, operand });
	const selectItem = () => {
		dispatch(projectActions.selectFiles({ projectId, selection: operand }));
	};

	return (
		<WorkspaceItemRow
			{...props}
			inert={!navigationIndexIncludes(navigationIndex, operand)}
			isSelected={isSelected}
			onSelect={selectItem}
		/>
	);
};

const TreeItem: FC<
	{
		projectId: string;
		operand: Operand;
		label: string;
		expanded?: boolean;
	} & useRender.ComponentProps<"div">
> = ({ projectId, operand, label, expanded, render, ...props }) => {
	const isSelected = useIsSelected({ projectId, operand });

	return useRender({
		render,
		defaultTagName: "div",
		props: mergeProps<"div">(props, {
			id: treeItemId(operand),
			role: "treeitem",
			"aria-label": label,
			"aria-selected": isSelected,
			"aria-expanded": expanded,
		}),
	});
};

const TreeChangeRow: FC<{
	change: TreeChange;
	operand: Operand;
	projectId: string;
	navigationIndex: NavigationIndex;
}> = ({ change, operand, projectId, navigationIndex }) => (
	<TreeItem
		projectId={projectId}
		operand={operand}
		label={changeLabel(change)}
		render={
			<OperationSourceC
				projectId={projectId}
				source={operand}
				render={
					<ItemRow projectId={projectId} operand={operand} navigationIndex={navigationIndex} />
				}
			/>
		}
	>
		<div className={workspaceItemRowStyles.itemRowLabel}>{changeLabel(change)}</div>
	</TreeItem>
);

const ConflictedFileRow: FC<{
	path: string;
	operand: Operand;
	projectId: string;
	navigationIndex: NavigationIndex;
}> = ({ path, operand, projectId, navigationIndex }) => {
	const label = `C ${path}`;
	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			label={label}
			render={
				<OperationSourceC
					projectId={projectId}
					source={operand}
					render={
						<ItemRow projectId={projectId} operand={operand} navigationIndex={navigationIndex} />
					}
				/>
			}
		>
			<div className={workspaceItemRowStyles.itemRowLabel}>{label}</div>
		</TreeItem>
	);
};

const ChangesFileRow: FC<{
	change: TreeChange;
	dependencyCommitIds: Array.NonEmptyArray<string> | undefined;

	projectId: string;
	navigationIndex: NavigationIndex;
}> = ({ change, dependencyCommitIds, projectId, navigationIndex }) => {
	const operand = fileOperand({ parent: changesFileParent, path: change.path });
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const isSelected = useIsSelected({ projectId, operand });
	const focusedPanel = useFocusedProjectPanel(projectId);

	const dispatch = useAppDispatch();
	const queryClient = useQueryClient();
	const openAbsorptionDialog = (target: AbsorptionTarget) => {
		// Before opening the dialog, warm cache to avoid showing loading states in
		// the dialog itself. This also ensures we don't show a stale absorption
		// plan whilst the dialog revalidates.
		void queryClient.prefetchQuery(absorptionPlanQueryOptions({ projectId, target })).then(() => {
			dispatch(projectActions.openAbsorptionDialog({ projectId, target }));
		});
	};

	const absorb = () => {
		openAbsorptionDialog({
			type: "treeChanges",
			subject: {
				changes: [change],
				assignedStackId: null,
			},
		});
	};

	useHotkey("A", absorb, {
		conflictBehavior: "allow",
		enabled: isSelected && focusedPanel === "files" && outlineMode._tag === "Default",
		meta: { group: "Changes", name: "Absorb" },
	});

	const menuItems: Array<NativeMenuItem> = [
		{
			_tag: "Item",
			label: "Absorb",
			onSelect: absorb,
		},
	];

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			label={changeLabel(change)}
			render={
				<OperationSourceC
					projectId={projectId}
					source={operand}
					render={
						<ItemRow projectId={projectId} operand={operand} navigationIndex={navigationIndex} />
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
				{changeLabel(change)}
			</div>
			{outlineMode._tag === "Default" && (
				<WorkspaceItemRowToolbar aria-label="File actions">
					{dependencyCommitIds && (
						<DependencyIndicatorButton
							projectId={projectId}
							commitIds={dependencyCommitIds}
							render={
								<Toolbar.Button
									type="button"
									className={workspaceItemRowStyles.itemRowToolbarButton}
								/>
							}
						>
							<DependencyIcon />
						</DependencyIndicatorButton>
					)}
					<Toolbar.Button
						type="button"
						className={workspaceItemRowStyles.itemRowToolbarButton}
						aria-label="File menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
					>
						<MenuTriggerIcon />
					</Toolbar.Button>
				</WorkspaceItemRowToolbar>
			)}
		</TreeItem>
	);
};
