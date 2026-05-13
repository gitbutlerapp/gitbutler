import uiStyles from "#ui/ui/ui.module.css";
import { FilesPanel } from "./FilesPanel.tsx";
import { applyBranchMutationOptions } from "#ui/api/mutations.ts";
import {
	headInfoQueryOptions,
	listBranchesQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import { useActiveElement } from "#ui/focus.ts";
import { focusPanel, Panel as PanelType, useFocusedProjectPanel } from "#ui/panels.ts";
import { isPanelVisible } from "#ui/panels/state.ts";
import {
	projectActions,
	selectProjectDialogState,
	selectProjectOutlineModeState,
	selectProjectPanelsState,
	selectProjectSelectionOutline,
} from "#ui/projects/state.ts";
import { ShortcutsBarPortal, TopBarActionsPortal } from "#ui/portals.tsx";
import { Keys } from "#ui/components/Keys.tsx";
import { ShortcutButton } from "#ui/components/ShortcutButton.tsx";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { isInputElement } from "#ui/commands/hotkeys.ts";
import { BranchListing, Segment, Stack } from "@gitbutler/but-sdk";
import { formatForDisplay, type Hotkey, type HotkeySequence } from "@tanstack/react-hotkeys";
import { useMutation, useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Match, Order } from "effect";
import { FC } from "react";
import { Group, Separator, useDefaultLayout } from "react-resizable-panels";
import { branchOperand, type BranchOperand } from "#ui/operands.ts";
import { PickerDialog, type PickerDialogGroup } from "#ui/ui/PickerDialog/PickerDialog.tsx";
import { DetailsPanel } from "./DetailsPanel.tsx";
import styles from "./WorkspacePage.module.css";
import { OutlinePanel } from "#ui/routes/project/$id/workspace/OutlinePanel.tsx";
import { classes } from "#ui/ui/classes.ts";
import {
	CommandFn,
	type CommandId,
	resolvedCommandHotkeys,
	useCommandHotkeys,
	useProjectCommands,
} from "#ui/commands/manager.ts";
import type { CommandGroup } from "#ui/commands/groups.ts";
import { getOperation, getOperations, operationLabel } from "#ui/operations/operation.ts";

type CommandPaletteItem = {
	id: CommandId;
	label: string;
	hotkeys?: Array<Hotkey | HotkeySequence>;
	commandFn: CommandFn;
};

const CommandPalette: FC<{
	open: boolean;
	onOpenChange: (open: boolean) => void;
}> = ({ open, onOpenChange }) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const focusedPanel = useFocusedProjectPanel(projectId);
	const [cmds, hotkeys] = useProjectCommands({ focusedPanel, projectId });
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const panelsState = useAppSelector((state) => selectProjectPanelsState(state, projectId));
	const selectionOutline = useAppSelector((state) =>
		selectProjectSelectionOutline(state, projectId),
	);
	const keyboardTransfer =
		outlineMode._tag === "Transfer" && outlineMode.value._tag === "Keyboard"
			? outlineMode.value
			: null;
	const keyboardOperations = keyboardTransfer
		? getOperations(keyboardTransfer.source, selectionOutline)
		: null;
	const keyboardOperation = keyboardTransfer
		? getOperation({
				source: keyboardTransfer.source,
				target: selectionOutline,
				operationType: keyboardTransfer.operationType,
			})
		: null;
	const operationConfirmLabel = keyboardOperation
		? operationLabel(keyboardOperation)
		: outlineMode._tag === "Absorb" && outlineMode.absorptionPlan.length > 0
			? "Confirm"
			: undefined;

	const item = (id: CommandId, label?: string): CommandPaletteItem | null => {
		const commandFn = cmds[id];
		if (label === undefined || !commandFn) return null;

		return {
			id,
			label,
			commandFn,
			hotkeys: resolvedCommandHotkeys(hotkeys[id]),
		};
	};

	const group = (
		value: CommandGroup,
		items: Array<CommandPaletteItem | null>,
	): PickerDialogGroup<CommandPaletteItem> | null => {
		const filteredItems = items
			.filter((item) => item !== null)
			.toSorted(Order.mapInput(Order.string, (cmd) => cmd.label));

		if (filteredItems.length === 0) return null;

		return { value, items: filteredItems };
	};

	const items = [
		group("Branches", [item("branch.apply", "Apply")]),
		group("Outline", [
			item("branch.select", "Select branch"),
			item("changes.select_changes", "Select changes"),
			item("changes.compose_message", "Compose commit message"),
			item("selection.move", "Move"),
			item("selection.cut", "Cut"),
			item("changes.commit_mode", "Commit"),
		]),
		group("Details", [
			item("details.toggle", isPanelVisible(panelsState, "details") ? "Close" : "Open"),
		]),
		group("Changes", [
			item("changes.focus_message", "Compose commit message"),
			item("changes.absorb", "Absorb"),
		]),
		group("Changes file", [item("changes_file.absorb", "Absorb")]),
		group("Commit", [
			item("commit.amend", "Amend"),
			item("commit.reword", "Reword"),
			item("commit.add_empty.above", "Add empty commit above"),
			item("commit.add_empty.below", "Add empty commit below"),
			item("commit.delete", "Delete commit"),
		]),
		group("Branch", [item("branch.rename", "Rename")]),
		group("Stack", [item("stack.unapply", "Unapply stack")]),
		group("Operation mode", [
			item(
				"operation.move_above",
				keyboardOperations?.moveAbove
					? `Select ${operationLabel(keyboardOperations.moveAbove)}`
					: undefined,
			),
			item(
				"operation.rub",
				keyboardOperations?.rub ? `Select ${operationLabel(keyboardOperations.rub)}` : undefined,
			),
			item(
				"operation.move_below",
				keyboardOperations?.moveBelow
					? `Select ${operationLabel(keyboardOperations.moveBelow)}`
					: undefined,
			),
			item("operation.confirm", operationConfirmLabel),
			item(
				"mode.cancel",
				outlineMode._tag === "Absorb" || outlineMode._tag === "Transfer" ? "Cancel" : undefined,
			),
		]),
	].flatMap((group) => (group ? [group] : []));

	const runCommand = (hotkey: CommandPaletteItem) => {
		onOpenChange(false);
		hotkey.commandFn();
	};

	return (
		<PickerDialog
			ariaLabel="Command palette"
			closeLabel="Close command palette"
			emptyLabel="No commands found."
			getItemKey={(x) => x.id}
			getItemLabel={(x) => x.label}
			getItemType={(x) => {
				// TODO: Render all hotkeys.
				const firstViable = x.hotkeys?.find((hk) => typeof hk === "string");
				return firstViable !== undefined && <Keys hotkey={firstViable} />;
			}}
			items={items}
			open={open}
			onOpenChange={onOpenChange}
			onSelectItem={runCommand}
			placeholder="Search commands…"
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
	const { data: headInfo } = useSuspenseQuery(headInfoQueryOptions(projectId));
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
					items: headInfo.stacks.flatMap(stackToBranchPickerOptions),
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
	const applyBranch = useMutation(applyBranchMutationOptions);
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

const TopBarActions: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const focusedPanel = useFocusedProjectPanel(projectId);
	const [cmds, hotkeys] = useProjectCommands({ focusedPanel, projectId });
	const panelsState = useAppSelector((state) => selectProjectPanelsState(state, projectId));

	return (
		<>
			<ShortcutButton
				className={uiStyles.button}
				hotkeys={resolvedCommandHotkeys(hotkeys["branch.apply"])}
				onClick={cmds["branch.apply"]}
			>
				Apply branch
			</ShortcutButton>
			<ShortcutButton
				className={uiStyles.button}
				hotkeys={resolvedCommandHotkeys(hotkeys["details.toggle"])}
				aria-pressed={isPanelVisible(panelsState, "details")}
				onClick={cmds["details.toggle"]}
			>
				Details
			</ShortcutButton>
		</>
	);
};

const isInputIgnoredHotkey = ({
	activeElement,
	hotkeyOpts,
}: {
	activeElement: Element | null;
	hotkeyOpts: { ignoreInputs?: boolean };
}): boolean =>
	hotkeyOpts.ignoreInputs !== false &&
	isInputElement(activeElement) &&
	activeElement !== document.documentElement;

const ShortcutsBar: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const focusedPanel = useFocusedProjectPanel(projectId);
	const [, hotkeys] = useProjectCommands({ focusedPanel, projectId });
	const activeElement = useActiveElement();
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));
	const selectionOutline = useAppSelector((state) =>
		selectProjectSelectionOutline(state, projectId),
	);
	const keyboardTransfer =
		outlineMode._tag === "Transfer" && outlineMode.value._tag === "Keyboard"
			? outlineMode.value
			: null;
	const keyboardOperation = keyboardTransfer
		? getOperation({
				source: keyboardTransfer.source,
				target: selectionOutline,
				operationType: keyboardTransfer.operationType,
			})
		: null;
	const shortcutBarLabels: Partial<Record<CommandId, string>> = {
		"branch.apply": "Apply",
		"branch.rename": "Rename",
		"branch.select": "Branch",
		"changes.absorb": "Absorb",
		"changes.commit_mode": "Commit",
		"changes.compose_message": "Compose commit message",
		"changes.focus_message": "Compose commit message",
		"changes.select_changes": "Changes",
		"changes_file.absorb": "Absorb",
		"command_palette.toggle": "Command palette",
		"commit.amend": "Amend",
		"commit.reword": "Reword",
		"details.toggle": "Details",
		"mode.cancel": "Cancel",
		"operation.confirm": keyboardOperation ? operationLabel(keyboardOperation) : "Confirm",
		"panel.focus_next": "Focus next panel",
		"panel.focus_previous": "Focus previous panel",
		"selection.cut": "Cut",
		"selection.move": "Move",
	};
	const visibleHotkeys = Object.entries(hotkeys)
		.flatMap(([id, hotkeys]) => {
			const commandId = id as CommandId;
			const label = shortcutBarLabels[commandId];
			if (label === undefined || label === "" || hotkeys.length === 0) return [];

			return hotkeys.flatMap((hk) =>
				// TODO: Render sequences too.
				"sequence" in hk ||
				hk.enabled === false ||
				isInputIgnoredHotkey({ activeElement, hotkeyOpts: hk })
					? []
					: {
							label,
							hotkey: formatForDisplay(hk.hotkey),
						},
			);
		})
		.toSorted(Order.mapInput(Order.string, (hk) => hk.hotkey));

	if (visibleHotkeys.length === 0) return null;

	return (
		<div className={styles.shortcutsBarContainer}>
			<span className={styles.shortcutsBarScope}>{focusedPanel ?? "Shortcuts"}</span>
			{visibleHotkeys.map((hotkey) => (
				<div key={hotkey.hotkey} className={styles.shortcutsBarItem}>
					<kbd className={styles.shortcutsBarKeys}>{formatForDisplay(hotkey.hotkey)}</kbd>
					<span className={styles.shortcutsBarName}>{hotkey.label}</span>
				</div>
			))}
		</div>
	);
};

const WorkspacePage: FC = () => {
	const dispatch = useAppDispatch();

	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const dialog = useAppSelector((state) => selectProjectDialogState(state, projectId));
	const panelsState = useAppSelector((state) => selectProjectPanelsState(state, projectId));
	const focusedPanel = useFocusedProjectPanel(projectId);
	const [cmds, commandHotkeys] = useProjectCommands({ focusedPanel, projectId });
	useCommandHotkeys(cmds, commandHotkeys);

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
			<TopBarActionsPortal>
				<TopBarActions />
			</TopBarActionsPortal>

			<ShortcutsBarPortal>
				<ShortcutsBar />
			</ShortcutsBarPortal>

			<Group className={styles.page} defaultLayout={defaultLayout} onLayoutChange={onLayoutChanged}>
				<OutlinePanel
					id={"outline" satisfies PanelType}
					minSize={400}
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
							minSize={400}
							defaultSize={400}
							groupResizeBehavior="preserve-pixel-size"
							tabIndex={0}
							className={classes(styles.panel, styles.panelPadding)}
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
							className={classes(styles.panel, styles.panelPadding)}
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

export const Route: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const { data: projects } = useSuspenseQuery(listProjectsQueryOptions);
	const project = projects.find((project) => project.id === projectId);
	if (!project) return <p>Project not found.</p>;

	return <WorkspacePage />;
};
