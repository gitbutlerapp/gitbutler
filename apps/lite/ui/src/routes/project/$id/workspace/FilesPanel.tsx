import {
	branchDiffQueryOptions,
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
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
import { Icon } from "#ui/components/Icon.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { mergeProps, Toast, useRender } from "@base-ui/react";
import { Toolbar } from "@base-ui/react/toolbar";
import { AbsorptionTarget, TreeChange } from "@gitbutler/but-sdk";
import { useMutation, useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Array, Match } from "effect";
import { ComponentProps, createContext, FC, ReactNode, Suspense, use, useEffect } from "react";
import styles from "./FilesPanel.module.css";
import workspaceItemRowStyles from "./WorkspaceItemRow.module.css";
import { WorkspaceItemRow, WorkspaceItemRowToolbar } from "./WorkspaceItemRow.tsx";
import { decodeRefName } from "#ui/api/ref-name.ts";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { getDependencyCommitIds, getHunkDependencyDiffsByPath } from "#ui/hunk.ts";
import { DependencyIndicatorButton } from "#ui/routes/project/$id/workspace/DependencyIndicatorButton.tsx";
import { focusPanel, useFocusedProjectPanel, useNavigationIndexHotkeys } from "#ui/panels.ts";
import {
	buildNavigationIndex,
	NavigationIndex,
	navigationIndexIncludes,
} from "#ui/workspace/navigation-index.ts";
import { changesFileHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import { assert } from "#ui/assert.ts";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { errorMessageForToast } from "#ui/errors.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";

const NavigationIndexContext = createContext<NavigationIndex | null>(null);

const useNavigationIndex = (projectId: string, parent: Operand, files: Array<Operand>) => {
	const dispatch = useAppDispatch();

	const navigationIndex = buildNavigationIndex([{ section: parent, children: files }]);

	const selection = useAppSelector((state) => selectProjectSelectionFiles(state, projectId));

	// Reset selection when it's no longer part of the workspace.
	//
	// React allows state updates on render, but not for external stores.
	// https://react.dev/learn/you-might-not-need-an-effect#adjusting-some-state-when-a-prop-changes
	useEffect(() => {
		if (!navigationIndexIncludes(navigationIndex, selection))
			dispatch(
				projectActions.selectFiles({
					projectId,
					selection: parent,
				}),
			);
	}, [navigationIndex, selection, projectId, dispatch, parent]);

	return navigationIndex;
};

const useFilesTreeHotkeys = ({
	navigationIndex,
	projectId,
}: {
	navigationIndex: NavigationIndex;
	projectId: string;
}) => {
	const selection = useAppSelector((state) => selectProjectSelectionFiles(state, projectId));
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const focusedPanel = useFocusedProjectPanel(projectId);
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));

	const dispatch = useAppDispatch();

	const select = (newItem: Operand) =>
		dispatch(projectActions.selectFiles({ projectId, selection: newItem }));

	const isChangesFileSelected = selection._tag === "File" && selection.parent._tag === "Changes";

	const absorbSelectedFile = () => {
		if (!isChangesFileSelected) return;

		const change = worktreeChanges?.changes.find((change) => change.path === selection.path);
		if (!change) return;

		dispatch(
			projectActions.enterAbsorbMode({
				projectId,
				source: selection,
				sourceTarget: {
					type: "treeChanges",
					subject: {
						changes: [change],
						assignedStackId: null,
					},
				},
			}),
		);
		focusPanel("outline");
	};

	useHotkeys([
		{
			hotkey: changesFileHotkeys.absorb.hotkey,
			callback: absorbSelectedFile,
			options: {
				conflictBehavior: "allow",
				enabled:
					isChangesFileSelected && focusedPanel === "files" && outlineMode._tag === "Default",
				meta: changesFileHotkeys.absorb.meta,
			},
		},
	]);

	useNavigationIndexHotkeys({
		focusedPanel,
		navigationIndex,
		projectId,
		group: "Files",
		panel: "files",
		select,
		selection,
	});
};

const CommitFilesTreePanel: FC<
	{ projectId: string; commit: CommitOperand } & ComponentProps<"div">
> = ({ projectId, commit, ...panelProps }) => {
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

	return (
		<FilesTreePanel {...panelProps} parent={parent} files={files}>
			{(() => {
				if (conflictedPaths.length === 0 && data.changes.length === 0)
					return (
						<div className={classes(workspaceItemRowStyles.itemRowEmpty, styles.item)}>
							No changes.
						</div>
					);

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
								/>
							))}

						{data.changes.length > 0 &&
							data.changes.map((change) => (
								<CommitFileRow
									commitId={commit.commitId}
									operand={fileOperand({
										parent: commitFileParent(commit),
										path: change.path,
									})}
									key={change.path}
									change={change}
									projectId={projectId}
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
	} & ComponentProps<"div">
> = ({ projectId, ...panelProps }) => {
	const { data: worktreeChanges } = useSuspenseQuery(changesInWorktreeQueryOptions(projectId));

	const parent = changesSectionOperand;

	const files = worktreeChanges.changes.map((change) =>
		fileOperand({ parent: changesFileParent, path: change.path }),
	);

	const hunkDependencyDiffsByPath = getHunkDependencyDiffsByPath(
		worktreeChanges.dependencies?.diffs ?? [],
	);

	return (
		<FilesTreePanel {...panelProps} parent={parent} files={files}>
			{worktreeChanges.changes.length === 0 ? (
				<div className={classes(workspaceItemRowStyles.itemRowEmpty, styles.item)}>No changes.</div>
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
	} & ComponentProps<"div">
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

	return (
		<FilesTreePanel {...panelProps} parent={parent} files={files}>
			{branchDiff.changes.length === 0 ? (
				<div className={classes(workspaceItemRowStyles.itemRowEmpty, styles.item)}>No changes.</div>
			) : (
				<div role="group">
					{branchDiff.changes.map((change) => (
						<BranchFileRow
							operand={fileOperand({
								parent: branchFileParent({ stackId, branchRef }),
								path: change.path,
							})}
							key={change.path}
							change={change}
							projectId={projectId}
						/>
					))}
				</div>
			)}
		</FilesTreePanel>
	);
};

export const FilesPanel: FC<ComponentProps<"div">> = (panelProps) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const outlineSelection = useAppSelector((state) =>
		selectProjectSelectionOutline(state, projectId),
	);

	return (
		<Suspense
			fallback={
				<div {...panelProps} className={classes(panelProps.className, styles.loading)}>
					Loading files…
				</div>
			}
		>
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
				Match.orElse(() => <div {...panelProps} />),
			)}
		</Suspense>
	);
};

const FilesTreePanel: FC<{ parent: Operand; files: Array<Operand> } & ComponentProps<"div">> = ({
	className,
	children,
	parent,
	files,
	...panelProps
}) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const navigationIndex = useNavigationIndex(projectId, parent, files);
	const selection = useAppSelector((state) => selectProjectSelectionFiles(state, projectId));

	useFilesTreeHotkeys({
		navigationIndex,
		projectId,
	});

	return (
		<NavigationIndexContext value={navigationIndex}>
			<div
				{...panelProps}
				tabIndex={0}
				role="tree"
				aria-activedescendant={treeItemId(selection)}
				className={classes(className, styles.tree)}
			>
				<TreeItem
					projectId={projectId}
					operand={parent}
					aria-label="All changes"
					aria-expanded
					className={workspaceItemRowStyles.section}
					render={<OperationSourceC projectId={projectId} selectionScope="files" source={parent} />}
				>
					<ItemRow projectId={projectId} operand={parent}>
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
			</div>
		</NavigationIndexContext>
	);
};

const useIsSelected = ({ projectId, operand }: { projectId: string; operand: Operand }): boolean =>
	useAppSelector((state) => {
		const selection = selectProjectSelectionFiles(state, projectId);

		return operandEquals(selection, operand);
	});

const treeItemId = (operand: Operand): string =>
	`files-treeitem-${encodeURIComponent(operandIdentityKey(operand))}`;

const useCopyPathMenuItem = (relativePath: string): NativeMenuItem => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);

	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");

	return nativeMenuItem({
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
	});
};

const useCommitUncommitChanges = () => {
	const dispatch = useAppDispatch();

	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: window.lite.commitUncommitChanges,
		onSuccess: async (response, input, _context, mutation) => {
			mutation.client.setQueryData(
				headInfoQueryOptions(input.projectId).queryKey,
				response.workspace.headInfo,
			);
			dispatch(
				projectActions.updateRewrittenCommitReferences({
					projectId: input.projectId,
					replacedCommits: response.workspace.replacedCommits,
					headInfo: response.workspace.headInfo,
				}),
			);
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to uncommit",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
};

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
		projectId: string;
		operand: Operand;
	} & Omit<ComponentProps<typeof WorkspaceItemRow>, "inert" | "isSelected" | "onSelect">
> = ({ projectId, operand, ...props }) => {
	const dispatch = useAppDispatch();
	const navigationIndex = assert(use(NavigationIndexContext));
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
			className={classes(props.className, styles.item)}
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

const CommitFileRow: FC<{
	commitId: string;
	change: TreeChange;
	operand: Operand;
	projectId: string;
}> = ({ commitId, change, operand, projectId }) => {
	const [label, strLabel] = changeLabel(change);
	const copyPathMenuItem = useCopyPathMenuItem(change.path);
	const commitUncommitChanges = useCommitUncommitChanges();
	const menuItems: Array<NativeMenuItem> = [
		copyPathMenuItem,
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Uncommit",
			enabled: !commitUncommitChanges.isPending,
			onSelect: () =>
				commitUncommitChanges.mutate({
					projectId,
					commitId,
					assignTo: null,
					changes: [createDiffSpec(change, [])],
					dryRun: false,
				}),
		}),
	];

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label={strLabel}
			render={
				<OperationSourceC
					projectId={projectId}
					selectionScope="files"
					source={operand}
					render={<ItemRow projectId={projectId} operand={operand} />}
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
		</TreeItem>
	);
};

const BranchFileRow: FC<{
	change: TreeChange;
	operand: Operand;
	projectId: string;
}> = ({ change, operand, projectId }) => {
	const [label, strLabel] = changeLabel(change);
	const copyPathMenuItem = useCopyPathMenuItem(change.path);
	const menuItems: Array<NativeMenuItem> = [copyPathMenuItem];

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label={strLabel}
			render={
				<OperationSourceC
					projectId={projectId}
					selectionScope="files"
					source={operand}
					render={<ItemRow projectId={projectId} operand={operand} />}
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
		</TreeItem>
	);
};

const ConflictedFileRow: FC<{
	path: string;
	operand: Operand;
	projectId: string;
}> = ({ path, operand, projectId }) => {
	const label = `C ${path}`;
	const copyPathMenuItem = useCopyPathMenuItem(path);
	const menuItems: Array<NativeMenuItem> = [copyPathMenuItem];

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label={label}
			render={
				<OperationSourceC
					projectId={projectId}
					selectionScope="files"
					source={operand}
					render={<ItemRow projectId={projectId} operand={operand} />}
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
		</TreeItem>
	);
};

const ChangesFileRow: FC<{
	change: TreeChange;
	dependencyCommitIds: Array.NonEmptyArray<string> | undefined;
	projectId: string;
}> = ({ change, dependencyCommitIds, projectId }) => {
	const operand = fileOperand({ parent: changesFileParent, path: change.path });
	const isSelected = useIsSelected({ projectId, operand });
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const dispatch = useAppDispatch();
	const enterAbsorbMode = (source: Operand, sourceTarget: AbsorptionTarget) => {
		dispatch(projectActions.enterAbsorbMode({ projectId, source, sourceTarget }));
		focusPanel("outline");
	};

	const absorb = () => {
		enterAbsorbMode(operand, {
			type: "treeChanges",
			subject: {
				changes: [change],
				assignedStackId: null,
			},
		});
	};

	const copyPathMenuItem = useCopyPathMenuItem(change.path);
	const menuItems: Array<NativeMenuItem> = [
		copyPathMenuItem,
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Absorb",
			enabled: true,
			accelerator: toElectronAccelerator(changesFileHotkeys.absorb.hotkey),
			onSelect: absorb,
		}),
	];

	const [label, strLabel] = changeLabel(change);

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label={strLabel}
			render={
				<OperationSourceC
					projectId={projectId}
					selectionScope="files"
					source={operand}
					render={<ItemRow projectId={projectId} operand={operand} />}
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

			{outlineMode._tag === "Default" && (
				<Toolbar.Root aria-label="File actions" render={<WorkspaceItemRowToolbar />}>
					{dependencyCommitIds && (
						<Toolbar.Button
							className={getButtonClassName({
								variant: isSelected ? "inverted" : "ghost",
								size: "small",
								iconOnly: true,
							})}
							render={
								<DependencyIndicatorButton projectId={projectId} commitIds={dependencyCommitIds} />
							}
						>
							<Icon name="link" />
						</Toolbar.Button>
					)}
					<Toolbar.Button
						aria-label="File menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getButtonClassName({
							variant: isSelected ? "inverted" : "ghost",
							size: "small",
							iconOnly: true,
						})}
					>
						<Icon name="kebab" />
					</Toolbar.Button>
				</Toolbar.Root>
			)}
		</TreeItem>
	);
};
