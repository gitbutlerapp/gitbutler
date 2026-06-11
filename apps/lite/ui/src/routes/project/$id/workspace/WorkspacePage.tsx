import {
	absorptionPlanQueryOptions,
	headInfoQueryOptions,
	listBranchesQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import {
	useApplyBranch,
	useBranchCreate,
	useRebaseAllStacks,
	useRestoreSnapshot,
} from "#ui/api/mutations.ts";
import { findBranchOperandByRef } from "#ui/api/ref-info.ts";
import { encodeBytes } from "#ui/api/ref-name.ts";
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
import { Kbd } from "#ui/components/Kbd.tsx";
import { globalHotkeys, workspaceHotkeys, type CommandGroup } from "#ui/hotkeys.ts";
import { stackToBottomRebaseUpdate } from "#ui/api/stack.ts";
import { type AppThunk, useAppDispatch, useAppSelector } from "#ui/store.ts";
import { BranchListing, RefInfo, Segment, Stack } from "@gitbutler/but-sdk";
import {
	getHotkeyManager,
	getSequenceManager,
	Hotkey,
	HotkeySequence,
	useHotkeys,
	useHotkeyRegistrations,
} from "@tanstack/react-hotkeys";
import {
	QueryErrorResetBoundary,
	useIsFetching,
	useIsMutating,
	useQueries,
	useQuery,
	useSuspenseQuery,
} from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Match, Order } from "effect";
import { type FC, Component, ReactNode } from "react";
import {
	branchOperand,
	changesSectionOperand,
	commitOperand,
	Operand,
	operandIdentityKey,
	stackOperand,
	type BranchOperand,
} from "#ui/operands.ts";
import { PickerDialog, type PickerDialogGroup } from "#ui/components/PickerDialog.tsx";
import { Details } from "./Details.tsx";
import styles from "./WorkspacePage.module.css";
import { OutlineTree } from "#ui/routes/project/$id/workspace/OutlineTree.tsx";
import { Button, Tooltip } from "@base-ui/react";
import { useActiveElement } from "#ui/focus.ts";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { filterNavigationItemsForOutlineMode } from "#ui/outline/mode.ts";
import { buildNavigationIndex } from "#ui/workspace/navigation-index.ts";
import { randomBranchRef } from "#ui/routes/project/$id/workspace/branchRef.ts";

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

type CommandPaletteItem = {
	group: CommandGroup;
	id: string;
	name: string;
	hotkey: Hotkey | HotkeySequence;
	type: "hotkey" | "sequence";
};

const groupCommandPaletteItems = (
	items: Array<CommandPaletteItem>,
): Array<PickerDialogGroup<CommandPaletteItem>> => {
	const grouped = Map.groupBy(items, (item) => item.group);

	return Array.from(grouped.entries())
		.toSorted(Order.mapInput(Order.string, ([group]) => group))
		.map(([group, items]) => ({
			value: group,
			items: items.toSorted(Order.mapInput(Order.string, (item) => item.name)),
		}));
};

const CommandPalette: FC<{
	open: boolean;
	onOpenChange: (open: boolean) => void;
}> = ({ open, onOpenChange }) => {
	const { hotkeys, sequences } = useHotkeyRegistrations();
	const hotkeyItems: Array<CommandPaletteItem> = [
		...hotkeys.flatMap((hotkey): CommandPaletteItem | [] =>
			hotkey.options.enabled !== false && hotkey.options.meta?.name !== undefined
				? {
						group: hotkey.options.meta.group,
						id: hotkey.id,
						name: hotkey.options.meta.name,
						hotkey: hotkey.hotkey,
						type: "hotkey",
					}
				: [],
		),
		...sequences.flatMap((sequence): CommandPaletteItem | [] =>
			sequence.options.enabled !== false && sequence.options.meta?.name !== undefined
				? {
						group: sequence.options.meta.group,
						id: sequence.id,
						name: sequence.options.meta.name,
						hotkey: sequence.sequence,
						type: "sequence",
					}
				: [],
		),
	];
	const items = groupCommandPaletteItems(hotkeyItems);

	const runHotkey = (item: CommandPaletteItem) => {
		onOpenChange(false);
		if (item.type === "hotkey") getHotkeyManager().triggerRegistration(item.id);
		else getSequenceManager().triggerSequence(item.id);
	};

	return (
		<PickerDialog
			ariaLabel="Command palette"
			closeLabel="Close command palette"
			emptyLabel="No hotkeys found."
			getItemKey={(x) => x.id}
			getItemLabel={(x) => x.name}
			getItemType={(x) => <Kbd hotkey={x.hotkey} />}
			items={items}
			open={open}
			onOpenChange={onOpenChange}
			onSelectItem={runHotkey}
			placeholder="Search hotkeys…"
		/>
	);
};

type BranchPickerOption = {
	id: string;
	label: string;
	branch: BranchOperand;
};

const segmentToBranchPickerOption = ({
	segment,
	stackId,
}: {
	segment: Segment;
	stackId: string;
}): BranchPickerOption | null => {
	const refName = segment.refName;
	if (!refName) return null;

	return {
		id: JSON.stringify([stackId, refName.fullNameBytes]),
		label: refName.displayName,
		branch: { stackId, branchRef: refName.fullNameBytes },
	};
};

const stackToBranchPickerOptions = (stack: Stack): Array<BranchPickerOption> => {
	// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
	const stackId = stack.id!;
	return stack.segments.flatMap((segment): Array<BranchPickerOption> => {
		const option = segmentToBranchPickerOption({ segment, stackId });
		return option ? [option] : [];
	});
};

const BranchPicker: FC<{
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onSelectBranch: (branch: BranchOperand) => void;
}> = ({ open, onOpenChange, onSelectBranch }) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const selectBranch = (option: BranchPickerOption) => {
		onOpenChange(false);
		onSelectBranch(option.branch);
	};

	return (
		<PickerDialog
			ariaLabel="Select branch"
			closeLabel="Close branch picker"
			emptyLabel="No results found."
			getItemKey={(x) => x.id}
			getItemLabel={(x) => x.label}
			getItemType={() => "Branch"}
			itemToStringValue={(x) => x.label}
			items={[
				{
					value: "Branches",
					items: headInfo?.stacks.flatMap(stackToBranchPickerOptions) ?? [],
				},
			]}
			open={open}
			onOpenChange={onOpenChange}
			onSelectItem={selectBranch}
			placeholder="Search for branches…"
		/>
	);
};

type ApplyBranchPickerOption = {
	branchRef: string;
	label: string;
	type: string;
};

const branchListingToApplyBranchPickerOptions = (
	branch: BranchListing,
): Array<ApplyBranchPickerOption> => {
	if (branch.hasLocal)
		return [
			{
				branchRef: `refs/heads/${branch.name}`,
				label: branch.name,
				type: "Local",
			},
		];

	return branch.remotes.map((remote) => ({
		branchRef: `refs/remotes/${remote}/${branch.name}`,
		label: branch.name,
		type: remote,
	}));
};

const ApplyBranchPicker: FC<{
	open: boolean;
	onOpenChange: (open: boolean) => void;
	projectId: string;
}> = ({ open, onOpenChange, projectId }) => {
	const branchesQuery = useQuery(
		listBranchesQueryOptions({ projectId, filter: { local: null, applied: false } }),
	);
	const items = (branchesQuery.data ?? []).flatMap(branchListingToApplyBranchPickerOptions);
	const applyBranch = useApplyBranch();
	const statusLabel =
		items.length === 0
			? branchesQuery.isPending
				? "Loading branches…"
				: branchesQuery.isError
					? "Unable to load branches."
					: undefined
			: undefined;

	const selectBranch = (option: ApplyBranchPickerOption) => {
		onOpenChange(false);
		applyBranch.mutate({ projectId, existingBranch: option.branchRef });
	};

	return (
		<PickerDialog
			ariaLabel="Apply branch"
			closeLabel="Close apply branch picker"
			emptyLabel="No available branches found."
			getItemKey={(x) => x.branchRef}
			getItemLabel={(x) => x.label}
			getItemType={(x) => x.type}
			itemToStringValue={(x) => x.label}
			items={[
				{
					value: "Available branches",
					items: (branchesQuery.data ?? []).flatMap(branchListingToApplyBranchPickerOptions),
				},
			]}
			open={open}
			onOpenChange={onOpenChange}
			onSelectItem={selectBranch}
			placeholder="Search for branches to apply…"
			statusLabel={statusLabel}
		/>
	);
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

		...(headInfo?.stacks.flatMap((stack) => {
			// oxlint-disable-next-line typescript/no-non-null-assertion -- [ref:stack-id-required]
			const stackId = stack.id!;
			return [
				stackOperand({ stackId }),
				...stack.segments.flatMap((segment) => segmentItems(stackId, segment)),
			];
		}) ?? []),
	];
};

const useOutlineNavigationIndex = ({
	projectId,
	absorptionTargetKeys,
}: {
	projectId: string;
	absorptionTargetKeys: ReadonlySet<string>;
}) => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const items = outlineNavigationItems(headInfo);
	const filteredItems = filterNavigationItemsForOutlineMode({
		items,
		outlineMode,
		absorptionTargetKeys,
	});
	const navigationIndex = buildNavigationIndex(filteredItems, operandIdentityKey);

	return navigationIndex;
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
	const createIndependentBranch = () => {
		const newRef = randomBranchRef();
		branchCreateMutation.mutate(
			{
				projectId,
				newRef,
				placement: { type: "independent" },
			},
			{
				onSuccess: (response) => {
					const newBranch = findBranchOperandByRef({
						headInfo: response.workspace.headInfo,
						branchRef: encodeBytes(newRef),
					});
					if (newBranch) selectBranch(newBranch);
				},
			},
		);
	};

	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const rebaseUpdates =
		headInfo?.stacks.flatMap((stack) => {
			const update = stackToBottomRebaseUpdate(stack);
			return update ? [update] : [];
		}) ?? [];
	const rebaseAllStacksMutation = useRebaseAllStacks({ projectId });
	const rebaseAllStacks = () => {
		rebaseAllStacksMutation.mutate(rebaseUpdates);
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
	const canRebaseAllStacks =
		outlineMode._tag === "Default" &&
		rebaseUpdates.length > 0 &&
		!rebaseAllStacksMutation.isPending;

	const canCreateIndependentBranch =
		outlineMode._tag === "Default" && !branchCreateMutation.isPending;

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
			hotkey: workspaceHotkeys.rebaseAllStacks.hotkey,
			callback: rebaseAllStacks,
			options: {
				conflictBehavior: "allow",
				enabled: canRebaseAllStacks,
				meta: workspaceHotkeys.rebaseAllStacks.meta,
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

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const selectedProject = projects.find((project) => project.id === projectId);
	if (!selectedProject) throw new Error("Could not find selected project");

	const rebaseAllLabel = "Integrate upstream changes by rebasing all stacks";

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
										aria-label={rebaseAllLabel}
										className={getButtonClassName({ iconOnly: true })}
										onClick={rebaseAllStacks}
										// We pass `disabled` here because we want to disable the button, not
										// the tooltip. Other props should be passed above.
										render={<Button focusableWhenDisabled disabled={!canRebaseAllStacks} />}
									>
										<Icon name="arrow-line-down" />
									</Tooltip.Trigger>
									<Tooltip.Portal>
										<Tooltip.Positioner sideOffset={4}>
											<Tooltip.Popup
												render={<TooltipPopup kbd={workspaceHotkeys.rebaseAllStacks.hotkey} />}
											>
												{rebaseAllLabel}
											</Tooltip.Popup>
										</Tooltip.Positioner>
									</Tooltip.Portal>
								</Tooltip.Root>

								<Tooltip.Root>
									<Tooltip.Trigger
										className={getButtonClassName({})}
										onClick={openApplyBranchPicker}
										// We pass `disabled` here because we want to disable the button, not
										// the tooltip. Other props should be passed above.
										render={<Button focusableWhenDisabled disabled={!canApplyBranch} />}
									>
										Apply branch
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

								<Tooltip.Root>
									<Tooltip.Trigger
										aria-label="Create independent branch"
										className={getButtonClassName({ iconOnly: true })}
										onClick={createIndependentBranch}
										render={<Button focusableWhenDisabled disabled={!canCreateIndependentBranch} />}
									>
										{branchCreateMutation.isPending ? (
											<Icon name="spinner" />
										) : (
											<Icon name="plus" />
										)}
									</Tooltip.Trigger>
									<Tooltip.Portal>
										<Tooltip.Positioner sideOffset={4}>
											<Tooltip.Popup
												render={
													<TooltipPopup kbd={workspaceHotkeys.createIndependentBranch.hotkey} />
												}
											>
												{workspaceHotkeys.createIndependentBranch.meta.name}
											</Tooltip.Popup>
										</Tooltip.Positioner>
									</Tooltip.Portal>
								</Tooltip.Root>
							</div>
						</header>

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
					outlineSelection={outlineSelection}
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
