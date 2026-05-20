import { FilesPanel } from "./FilesPanel.tsx";
import uiStyles from "#ui/ui/ui.module.css";
import {
	headInfoQueryOptions,
	listBranchesQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import {
	focusAdjacentPanel,
	focusPanel,
	Panel as PanelType,
	useFocusedProjectPanel,
} from "#ui/panels.ts";
import { isPanelVisible } from "#ui/panels/state.ts";
import {
	projectActions,
	selectProjectDialogState,
	selectProjectPanelsState,
} from "#ui/projects/state.ts";
import { Keys } from "#ui/components/Keys.tsx";
import { globalHotkeys, workspaceHotkeys, type CommandGroup } from "#ui/hotkeys.ts";
import { type AppThunk, useAppDispatch, useAppSelector } from "#ui/store.ts";
import { BranchListing, Segment, Stack } from "@gitbutler/but-sdk";
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
	useMutation,
	useQuery,
	useSuspenseQuery,
} from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Match, Order } from "effect";
import { type FC, Component, ReactNode } from "react";
import { Group, Separator, useDefaultLayout } from "react-resizable-panels";
import { branchOperand, type BranchOperand } from "#ui/operands.ts";
import { PickerDialog, type PickerDialogGroup } from "#ui/ui/PickerDialog/PickerDialog.tsx";
import { DetailsPanel } from "./DetailsPanel.tsx";
import styles from "./WorkspacePage.module.css";
import { OutlinePanel } from "#ui/routes/project/$id/workspace/OutlinePanel.tsx";
import { classes } from "#ui/ui/classes.ts";
import { Toast } from "@base-ui/react";
import { errorMessageForToast } from "#ui/errors.ts";

type CommandPaletteItem = {
	group: CommandGroup;
	id: string;
	name: string;
	hotkey: Hotkey | HotkeySequence;
	type: "hotkey" | "sequence";
};

const toggleProjectPanel =
	({
		projectId,
		panel,
		focusedPanel,
	}: {
		projectId: string;
		panel: PanelType;
		focusedPanel: PanelType | null;
	}): AppThunk =>
	(dispatch, getState) => {
		const panelsState = selectProjectPanelsState(getState(), projectId);

		if (focusedPanel === panel && isPanelVisible(panelsState, panel)) {
			const panelIndex = panelsState.visiblePanels.indexOf(panel);
			const nextPanel = panelsState.visiblePanels[panelIndex - 1];
			if (nextPanel !== undefined) focusPanel(nextPanel);
		}

		dispatch(projectActions.togglePanel({ projectId, panel }));
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
			getItemType={(x) => <Keys hotkey={x.hotkey} />}
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
	const toastManager = Toast.useToastManager();
	const applyBranch = useMutation({
		mutationFn: window.lite.apply,
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to apply branch",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
	});
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
	const dialog = useAppSelector((state) => selectProjectDialogState(state, projectId));
	const panelsState = useAppSelector((state) => selectProjectPanelsState(state, projectId));
	const focusedPanel = useFocusedProjectPanel(projectId);

	const togglePanel = (panel: PanelType) => {
		dispatch(toggleProjectPanel({ projectId, panel, focusedPanel }));
	};

	useHotkeys([
		{
			hotkey: globalHotkeys.commandPalette.hotkey,
			callback: () => {
				if (dialog._tag === "CommandPalette") dispatch(projectActions.closeDialog({ projectId }));
				else dispatch(projectActions.openCommandPalette({ projectId, focusedPanel }));
			},
			options: {
				conflictBehavior: "allow",
				meta: globalHotkeys.commandPalette.meta,
			},
		},
		{
			hotkey: workspaceHotkeys.applyBranch.hotkey,
			callback: () => {
				dispatch(projectActions.openApplyBranchPicker({ projectId }));
			},
			options: {
				conflictBehavior: "allow",
				meta: workspaceHotkeys.applyBranch.meta,
			},
		},
		{
			hotkey: workspaceHotkeys.toggleFilesPanel.hotkey,
			callback: () => {
				togglePanel("files");
			},
			options: {
				conflictBehavior: "allow",
				meta: workspaceHotkeys.toggleFilesPanel.meta,
			},
		},
		{
			hotkey: workspaceHotkeys.focusPreviousPanel.hotkey,
			callback: () => {
				focusAdjacentPanel(-1, panelsState.visiblePanels);
			},
			options: {
				conflictBehavior: "allow",
				enabled: focusedPanel !== null,
				meta: workspaceHotkeys.focusPreviousPanel.meta,
			},
		},
		{
			hotkey: workspaceHotkeys.focusNextPanel.hotkey,
			callback: () => {
				focusAdjacentPanel(1, panelsState.visiblePanels);
			},
			options: {
				conflictBehavior: "allow",
				enabled: focusedPanel !== null,
				meta: workspaceHotkeys.focusNextPanel.meta,
			},
		},
	]);
};

const WorkspacePage: FC = () => {
	const dispatch = useAppDispatch();

	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const dialog = useAppSelector((state) => selectProjectDialogState(state, projectId));
	const panelsState = useAppSelector((state) => selectProjectPanelsState(state, projectId));
	const focusedPanel = useFocusedProjectPanel(projectId);

	useWorkspaceHotkeys(projectId);

	const { defaultLayout, onLayoutChanged } = useDefaultLayout({
		id: `project:${projectId}:layout`,
		panelIds: panelsState.visiblePanels,
	});

	const selectBranch = (branch: BranchOperand) => {
		dispatch(
			projectActions.selectOutline({
				projectId,
				selection: branchOperand(branch),
			}),
		);
		focusPanel("outline");
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
		if (open) dispatch(projectActions.openCommandPalette({ projectId, focusedPanel }));
		else dispatch(projectActions.closeDialog({ projectId }));
	};

	return (
		<>
			<Group className={styles.page} defaultLayout={defaultLayout} onLayoutChange={onLayoutChanged}>
				<OutlinePanel
					id={"outline" satisfies PanelType}
					minSize={300}
					defaultSize={500}
					groupResizeBehavior="preserve-pixel-size"
					tabIndex={0}
					className={styles.panel}
					elementRef={(el) => el?.focus({ focusVisible: false })}
				/>
				{isPanelVisible(panelsState, "files") && (
					<>
						<Separator className={styles.panelResizeHandle} />
						<FilesPanel
							id={"files" satisfies PanelType}
							minSize={250}
							defaultSize={400}
							groupResizeBehavior="preserve-pixel-size"
							tabIndex={0}
							className={classes(styles.panel, styles.filesPanel)}
						/>
					</>
				)}
				{isPanelVisible(panelsState, "details") && (
					<>
						<Separator className={styles.panelResizeHandle} />
						<DetailsPanel
							id={"details" satisfies PanelType}
							minSize={400}
							tabIndex={0}
							className={classes(styles.panel, styles.detailsPanel)}
						/>
					</>
				)}
			</Group>

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
					<button type="button" className={uiStyles.button} onClick={() => this.handleRetry()}>
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
