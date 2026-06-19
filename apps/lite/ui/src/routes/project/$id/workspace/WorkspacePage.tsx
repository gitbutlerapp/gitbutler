import {
	absorptionPlanQueryOptions,
	headInfoQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import {
	useBranchCheckoutNew,
	useBranchCreate,
	useWorkspaceIntegrateUpstream,
	useRestoreSnapshot,
} from "#ui/api/mutations.ts";
import { findBranchOperandByRef } from "#ui/api/ref-info.ts";
import {
	focusAdjacentSelectionScope,
	focusSelectionScope,
	getFocusedSelectionScope,
	SelectionScope,
	useOutlineSelection,
} from "#ui/selection-scopes.ts";
import {
	projectActions,
	selectProjectDetailsFullscreen,
	selectProjectDialogState,
	selectProjectFilesVisible,
	selectProjectOutlineModeState,
} from "#ui/projects/state.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { globalHotkeys, workspaceHotkeys } from "#ui/hotkeys.ts";
import { stackBottomRelativeTo } from "#ui/api/stack.ts";
import { type AppThunk, useAppDispatch, useAppSelector } from "#ui/store.ts";
import { BottomUpdate, RefInfo, Segment } from "@gitbutler/but-sdk";
import { useHotkeys } from "@tanstack/react-hotkeys";
import {
	QueryErrorResetBoundary,
	useIsFetching,
	useIsMutating,
	useQueries,
	useQuery,
	useSuspenseQuery,
} from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Match } from "effect";
import { type FC, Component, ReactNode, useDeferredValue } from "react";
import {
	branchOperand,
	changesSectionOperand,
	commitOperand,
	Operand,
	operandContains,
	operandEquals,
	operandIdentityKey,
	stackOperand,
	type BranchOperand,
} from "#ui/operands.ts";
import { Details } from "./Details.tsx";
import styles from "./WorkspacePage.module.css";
import { OutlineTree } from "#ui/routes/project/$id/workspace/OutlineTree.tsx";
import { Button, Toggle, ToggleGroup, Tooltip } from "@base-ui/react";
import { useActiveElement } from "#ui/focus.ts";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { buildIndexByKey, NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { reverse } from "effect/Array";
import { getOperations } from "#ui/operations/operation.ts";
import { ToggleGroupStyles, ToggleStyles } from "#ui/components/ToggleGroup.tsx";
import { ApplyBranchPicker } from "./ApplyBranchPicker.tsx";
import { BranchPicker } from "./BranchPicker.tsx";
import { CommandPalette } from "./CommandPalette.tsx";

const toggleFiles =
	({
		projectId,
		focusedSelectionScope,
		outlineVisible,
	}: {
		projectId: string;
		focusedSelectionScope: SelectionScope | null;
		outlineVisible: boolean;
	}): AppThunk =>
	(dispatch, getState) => {
		const filesVisible = selectProjectFilesVisible(getState(), projectId);

		if (focusedSelectionScope === "files" && filesVisible)
			focusSelectionScope(outlineVisible ? "outline" : "diff");

		dispatch(projectActions.toggleFiles({ projectId }));
	};

const useWorkspaceHotkeys = (projectId: string) => {
	const dispatch = useAppDispatch();
	const detailsFullscreen = useAppSelector((state) =>
		selectProjectDetailsFullscreen(state, projectId),
	);
	const dialog = useAppSelector((state) => selectProjectDialogState(state, projectId));
	const filesVisible = useAppSelector((state) => selectProjectFilesVisible(state, projectId));
	const activeElement = useActiveElement();
	const focusedSelectionScope = getFocusedSelectionScope(activeElement);
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const outlineVisible = !detailsFullscreen;

	const restoreSnapshotMutation = useRestoreSnapshot({ projectId });

	useHotkeys([
		{
			hotkey: globalHotkeys.redo.hotkey,
			callback: () => restoreSnapshotMutation.mutate("redo"),
			options: {
				enabled: outlineMode._tag === "Default" && !restoreSnapshotMutation.isPending,
				meta: globalHotkeys.redo.meta,
				ignoreInputs: true,
			},
		},
		{
			hotkey: globalHotkeys.undo.hotkey,
			callback: () => restoreSnapshotMutation.mutate("undo"),
			options: {
				enabled: outlineMode._tag === "Default" && !restoreSnapshotMutation.isPending,
				meta: globalHotkeys.undo.meta,
				ignoreInputs: true,
			},
		},
		{
			hotkey: globalHotkeys.commandPalette.hotkey,
			callback: () => {
				if (dialog._tag === "CommandPalette") dispatch(projectActions.closeDialog({ projectId }));
				else dispatch(projectActions.openCommandPalette({ projectId }));
			},
			options: {
				conflictBehavior: "allow",
				meta: globalHotkeys.commandPalette.meta,
			},
		},
		{
			hotkey: workspaceHotkeys.toggleFiles.hotkey,
			callback: () => {
				dispatch(toggleFiles({ projectId, focusedSelectionScope, outlineVisible }));
			},
			options: {
				conflictBehavior: "allow",
				meta: workspaceHotkeys.toggleFiles.meta,
			},
		},
		{
			hotkey: workspaceHotkeys.focusPreviousSelectionScope.hotkey,
			callback: () => {
				focusAdjacentSelectionScope({ filesVisible, offset: -1, outlineVisible });
			},
			options: {
				conflictBehavior: "allow",
				meta: workspaceHotkeys.focusPreviousSelectionScope.meta,
			},
		},
		{
			hotkey: workspaceHotkeys.focusNextSelectionScope.hotkey,
			callback: () => {
				focusAdjacentSelectionScope({ filesVisible, offset: 1, outlineVisible });
			},
			options: {
				conflictBehavior: "allow",
				meta: workspaceHotkeys.focusNextSelectionScope.meta,
			},
		},
	]);
};

const ActivitySpinner: FC = () => {
	const fetchingCount = useIsFetching();
	const mutatingCount = useIsMutating();

	const isFetching = fetchingCount > 0;
	const isMutating = mutatingCount > 0;

	const status = Match.value({ isFetching, isMutating }).pipe(
		Match.when({ isFetching: true, isMutating: true }, () => "Syncing"),
		Match.when({ isFetching: true }, () => "Loading"),
		Match.when({ isMutating: true }, () => "Saving"),
		Match.orElse(() => null),
	);

	return status !== null && <Icon name="spinner" aria-label={status} />;
};

const outlineNavigationItems = (headInfo: RefInfo | undefined): Array<Operand> => {
	const segmentItems = (stackId: string, segment: Segment): Array<Operand> => [
		...(segment.refName
			? [branchOperand({ stackId, branchRef: segment.refName.fullNameBytes })]
			: []),
		...segment.commits.map((commit) => commitOperand({ stackId, commitId: commit.id })),
	];

	return [
		changesSectionOperand,

		...reverse(headInfo?.stacks ?? []).flatMap((stack) => {
			// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
			const stackId = stack.id!;
			return [
				stackOperand({ stackId }),
				...stack.segments.flatMap((segment) => segmentItems(stackId, segment)),
			];
		}),
	];
};

const hasAnyOperation = (source: Operand, target: Operand) => {
	const operations = getOperations(source, target);
	return !!operations.into || !!operations.above || !!operations.below;
};

const useOutlineNavigationIndex = ({
	projectId,
	absorptionTargetKeys,
}: {
	projectId: string;
	absorptionTargetKeys: ReadonlySet<string>;
}): NavigationIndex<Operand> => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const items = outlineNavigationItems(headInfo);
	const filteredItems = Match.value(outlineMode).pipe(
		Match.tagsExhaustive({
			Default: () => items,
			Absorb: (activeMode) =>
				items.filter(
					(operand) =>
						operandContains(operand, activeMode.source) ||
						absorptionTargetKeys.has(operandIdentityKey(operand)),
				),
			Transfer: (activeMode) =>
				items.filter(
					(operand) =>
						operandContains(operand, activeMode.value.source) ||
						hasAnyOperation(activeMode.value.source, operand),
				),
			RenameBranch: (x) =>
				items.filter((operand) => operandEquals(operand, branchOperand(x.operand))),
			RewordCommit: (x) =>
				items.filter((operand) => operandEquals(operand, commitOperand(x.operand))),
		}),
	);
	const indexByKey = buildIndexByKey(filteredItems, operandIdentityKey);

	return { items: filteredItems, indexByKey };
};

const WorkspacePage: FC = () => {
	const dispatch = useAppDispatch();

	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const detailsFullscreen = useAppSelector((state) =>
		selectProjectDetailsFullscreen(state, projectId),
	);
	const dialog = useAppSelector((state) => selectProjectDialogState(state, projectId));
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	useWorkspaceHotkeys(projectId);

	const selectBranch = (branch: BranchOperand) => {
		dispatch(
			projectActions.selectOutline({
				projectId,
				selection: branchOperand(branch),
			}),
		);
		focusSelectionScope("outline");
	};

	const setBranchPickerOpen = (open: boolean) => {
		if (open) dispatch(projectActions.openBranchPicker({ projectId }));
		else dispatch(projectActions.closeDialog({ projectId }));
	};

	const setApplyBranchPickerOpen = (open: boolean) => {
		if (open) dispatch(projectActions.openApplyBranchPicker({ projectId }));
		else dispatch(projectActions.closeDialog({ projectId }));
	};

	const setCommandPaletteOpen = (open: boolean) => {
		if (open) dispatch(projectActions.openCommandPalette({ projectId }));
		else dispatch(projectActions.closeDialog({ projectId }));
	};

	const openApplyBranchPicker = () => {
		dispatch(projectActions.openApplyBranchPicker({ projectId }));
	};

	const branchCreateMutation = useBranchCreate();
	const branchCheckoutNewMutation = useBranchCheckoutNew();
	const createIndependentBranch = () => {
		branchCreateMutation.mutate(
			{
				projectId,
				newRef: null,
				placement: { type: "independent" },
			},
			{
				onSuccess: (response) => {
					const newBranch = findBranchOperandByRef({
						headInfo: response.workspace.headInfo,
						branchRef: response.newRef.fullNameBytes,
					});
					if (newBranch) selectBranch(newBranch);
				},
			},
		);
	};
	const resetWorkspace = () => {
		branchCheckoutNewMutation.mutate(
			{ projectId, name: null },
			{
				onSuccess: (response) => {
					const workspaceRef = response.workspace.headInfo.workspaceRef;
					if (!workspaceRef) return;

					const newBranch = findBranchOperandByRef({
						headInfo: response.workspace.headInfo,
						branchRef: workspaceRef.fullNameBytes,
					});
					if (newBranch) selectBranch(newBranch);
				},
			},
		);
	};

	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const rebaseUpdates =
		headInfo?.stacks.flatMap((stack): Array<BottomUpdate> => {
			const relativeTo = stackBottomRelativeTo(stack);
			return relativeTo ? [{ kind: "rebase", selector: relativeTo }] : [];
		}) ?? [];
	const workspaceIntegrateUpstreamMutation = useWorkspaceIntegrateUpstream();
	const updateWorkspace = () => {
		workspaceIntegrateUpstreamMutation.mutate({ projectId, updates: rebaseUpdates, dryRun: false });
	};
	const toggleDetailsFullscreen = () => {
		if (
			!detailsFullscreen &&
			getFocusedSelectionScope(document.activeElement) === ("outline" satisfies SelectionScope)
		)
			requestAnimationFrame(() => focusSelectionScope("diff"));

		dispatch(projectActions.toggleDetailsFullscreen({ projectId }));
	};
	// This should be false if all stacks are up-to-date, but we're currently
	// lacking this information:
	// https://linear.app/gitbutler/issue/GB-1560/add-information-about-the-relation-to-the-upstream-to-the-head-info
	const canUpdateWorkspace =
		outlineMode._tag === "Default" &&
		rebaseUpdates.length > 0 &&
		!workspaceIntegrateUpstreamMutation.isPending;

	const canCreateIndependentBranch =
		outlineMode._tag === "Default" && !branchCreateMutation.isPending;

	const canResetWorkspace = outlineMode._tag === "Default" && !branchCheckoutNewMutation.isPending;

	const canApplyBranch = outlineMode._tag === "Default";

	useHotkeys([
		{
			hotkey: workspaceHotkeys.applyBranch.hotkey,
			callback: openApplyBranchPicker,
			options: {
				conflictBehavior: "allow",
				meta: workspaceHotkeys.applyBranch.meta,
				enabled: canApplyBranch,
			},
		},
		{
			hotkey: workspaceHotkeys.createIndependentBranch.hotkey,
			callback: createIndependentBranch,
			options: {
				conflictBehavior: "allow",
				enabled: canCreateIndependentBranch,
				meta: workspaceHotkeys.createIndependentBranch.meta,
				ignoreInputs: true,
				requireReset: true,
			},
		},
		{
			hotkey: workspaceHotkeys.updateWorkspace.hotkey,
			callback: updateWorkspace,
			options: {
				conflictBehavior: "allow",
				enabled: canUpdateWorkspace,
				meta: workspaceHotkeys.updateWorkspace.meta,
				ignoreInputs: true,
			},
		},
		{
			hotkey: workspaceHotkeys.toggleDetailsFullscreen.hotkey,
			callback: toggleDetailsFullscreen,
			options: {
				conflictBehavior: "allow",
				meta: workspaceHotkeys.toggleDetailsFullscreen.meta,
				ignoreInputs: true,
			},
		},
		{
			hotkey: "Escape",
			callback: toggleDetailsFullscreen,
			options: {
				conflictBehavior: "allow",
				enabled: detailsFullscreen,
			},
		},
	]);

	const absorptionPlanTarget = Match.value(outlineMode).pipe(
		Match.tag("Absorb", ({ sourceTarget }) => sourceTarget),
		Match.orElse(() => null),
	);
	const [absorptionPlanQuery] = useQueries({
		queries: (absorptionPlanTarget ? [absorptionPlanTarget] : []).map((target) =>
			absorptionPlanQueryOptions({ projectId, target }),
		),
	});
	const absorptionTargetKeys = new Set(
		absorptionPlanQuery?.data?.map(({ stackId, commitId }) =>
			operandIdentityKey(commitOperand({ stackId, commitId })),
		),
	);

	const outlineNavigationIndex = useOutlineNavigationIndex({
		projectId,
		absorptionTargetKeys,
	});

	const outlineSelection = useOutlineSelection({
		projectId,
		navigationIndex: outlineNavigationIndex,
	});

	const deferredOutlineSelection = useDeferredValue(outlineSelection);

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");

	return (
		<>
			<div className={classes(styles.page, detailsFullscreen && styles.pageDetailsFullscreen)}>
				{!detailsFullscreen && (
					<div className={styles.outlinePanel}>
						<header className={styles.workspaceControls}>
							<div className={styles.workspaceControlsLeft}>
								<h1 className={classes("text-15", "text-bold", styles.workspaceName)}>
									{selectedProject.title}
								</h1>
								<ActivitySpinner />
							</div>

							<div className={styles.workspaceControlsActions}>
								<Tooltip.Root>
									<Tooltip.Trigger
										aria-label="Switch to new branch"
										className={getButtonClassName({ iconOnly: true })}
										onClick={resetWorkspace}
										// We pass `disabled` here because we want to disable the button, not
										// the tooltip. Other props should be passed above.
										render={<Button focusableWhenDisabled disabled={!canResetWorkspace} />}
									>
										{branchCheckoutNewMutation.isPending ? (
											<Icon name="spinner" />
										) : (
											<Icon name="undo" />
										)}
									</Tooltip.Trigger>
									<Tooltip.Portal>
										<Tooltip.Positioner sideOffset={4}>
											<Tooltip.Popup render={<TooltipPopup />}>Switch to new branch</Tooltip.Popup>
										</Tooltip.Positioner>
									</Tooltip.Portal>
								</Tooltip.Root>

								<Tooltip.Root>
									<Tooltip.Trigger
										aria-label={workspaceHotkeys.updateWorkspace.meta.name}
										className={getButtonClassName({ iconOnly: true })}
										onClick={updateWorkspace}
										// We pass `disabled` here because we want to disable the button, not
										// the tooltip. Other props should be passed above.
										render={<Button focusableWhenDisabled disabled={!canUpdateWorkspace} />}
									>
										<Icon name="arrow-line-down" />
									</Tooltip.Trigger>
									<Tooltip.Portal>
										<Tooltip.Positioner sideOffset={4}>
											<Tooltip.Popup
												render={<TooltipPopup kbd={workspaceHotkeys.updateWorkspace.hotkey} />}
											>
												{workspaceHotkeys.updateWorkspace.meta.name}
											</Tooltip.Popup>
										</Tooltip.Positioner>
									</Tooltip.Portal>
								</Tooltip.Root>

								<Tooltip.Root>
									<Tooltip.Trigger
										aria-label={workspaceHotkeys.applyBranch.meta.name}
										className={getButtonClassName({ iconOnly: true })}
										onClick={openApplyBranchPicker}
										// We pass `disabled` here because we want to disable the button, not
										// the tooltip. Other props should be passed above.
										render={<Button focusableWhenDisabled disabled={!canApplyBranch} />}
									>
										<Icon name="plus" />
									</Tooltip.Trigger>
									<Tooltip.Portal>
										<Tooltip.Positioner sideOffset={4}>
											<Tooltip.Popup
												render={<TooltipPopup kbd={workspaceHotkeys.applyBranch.hotkey} />}
											>
												{workspaceHotkeys.applyBranch.meta.name}
											</Tooltip.Popup>
										</Tooltip.Positioner>
									</Tooltip.Portal>
								</Tooltip.Root>
							</div>
						</header>

						<div className={styles.navContainer}>
							<ToggleGroup
								render={<ToggleGroupStyles />}
								aria-label="Navigation"
								value={["workspace"]}
							>
								<Toggle render={<ToggleStyles />} value="workspace">
									<Icon name="workbench" />
									Workspace
								</Toggle>
								<Toggle render={<ToggleStyles />} value="upstream" disabled>
									<Icon name="inbox" />
									Upstream
								</Toggle>
								<Toggle render={<ToggleStyles />} value="branches" disabled>
									<Icon name="branch" />
									Branches
								</Toggle>
							</ToggleGroup>
						</div>

						<OutlineTree
							id={"outline" satisfies SelectionScope}
							data-selection-scope
							tabIndex={0}
							navigationIndex={outlineNavigationIndex}
							absorptionTargetKeys={absorptionTargetKeys}
							// Focus on page load.
							ref={(el) => {
								// Don't steal focus if this component is mounted later on.
								if (document.activeElement !== document.body) return;

								el?.focus({ focusVisible: false });
							}}
						/>
					</div>
				)}

				<Details
					key={deferredOutlineSelection ? operandIdentityKey(deferredOutlineSelection) : null}
					style={{ opacity: deferredOutlineSelection !== outlineSelection ? 0.5 : 1 }}
					outlineSelection={deferredOutlineSelection}
					detailsFullscreen={detailsFullscreen}
					onDetailsFullscreenChange={(fullscreen) =>
						dispatch(projectActions.setDetailsFullscreen({ projectId, fullscreen }))
					}
				/>
			</div>

			{Match.value(dialog).pipe(
				Match.tagsExhaustive({
					None: () => null,
					ApplyBranchPicker: () => (
						<ApplyBranchPicker open onOpenChange={setApplyBranchPickerOpen} projectId={projectId} />
					),
					BranchPicker: () => (
						<BranchPicker open onOpenChange={setBranchPickerOpen} onSelectBranch={selectBranch} />
					),
					CommandPalette: () => <CommandPalette open onOpenChange={setCommandPaletteOpen} />,
				}),
			)}
		</>
	);
};

class WorkspacePageErrorBoundary extends Component<
	{
		onReset: () => void;
		children: ReactNode;
	},
	{
		error: Error | null;
	}
> {
	state = { error: null as Error | null };

	static getDerivedStateFromError(error: unknown) {
		return {
			error:
				error instanceof Error
					? error
					: new Error(typeof error === "string" ? error : JSON.stringify(error)),
		};
	}

	handleRetry(): void {
		this.props.onReset();
		this.setState({ error: null });
	}

	render(): ReactNode {
		if (!this.state.error) return this.props.children;

		return (
			<div className={styles.error}>
				<h1 className={styles.errorTitle}>Something went wrong.</h1>
				<div className={styles.errorActions}>
					<button
						type="button"
						className={getButtonClassName({})}
						onClick={() => this.handleRetry()}
					>
						Retry
					</button>
				</div>
				<code className={styles.errorMessage}>{this.state.error.message}</code>
			</div>
		);
	}
}

export const Route: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const project = projects.find((project) => project.id === projectId);
	if (!project) return <p className={styles.notFound}>Project not found.</p>;

	return (
		<QueryErrorResetBoundary>
			{({ reset }) => (
				<WorkspacePageErrorBoundary onReset={reset}>
					<WorkspacePage />
				</WorkspacePageErrorBoundary>
			)}
		</QueryErrorResetBoundary>
	);
};
