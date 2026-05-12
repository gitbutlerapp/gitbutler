import uiStyles from "#ui/ui/ui.module.css";
import { FilesPanel } from "./FilesPanel.tsx";
import { applyBranchMutationOptions } from "#ui/api/mutations.ts";
import {
	headInfoQueryOptions,
	listBranchesQueryOptions,
	listProjectsQueryOptions,
} from "#ui/api/queries.ts";
import { useActiveElement } from "#ui/focus.ts";
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
import { ShortcutsBarPortal, TopBarActionsPortal } from "#ui/portals.tsx";
import { Keys } from "#ui/components/Keys.tsx";
import { ShortcutButton } from "#ui/components/ShortcutButton.tsx";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { isInputElement } from "#ui/commands/hotkeys.ts";
import { BranchListing, Segment, Stack } from "@gitbutler/but-sdk";
import {
	formatForDisplay,
	Hotkey,
	HotkeyOptions,
	HotkeySequence,
	normalizeRegisterableHotkey,
} from "@tanstack/react-hotkeys";
import { useMutation, useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";
import { Match, Order } from "effect";
import { FC } from "react";
import { Group, Separator, useDefaultLayout } from "react-resizable-panels";
import { branchOperand, type BranchOperand } from "#ui/operands.ts";
import { PickerDialog, type PickerDialogGroup } from "#ui/ui/PickerDialog/PickerDialog.tsx";
import { DetailsPanel } from "./DetailsPanel.tsx";
import styles from "./WorkspacePage.module.css";
import type { CommandGroup } from "#ui/commands/groups.ts";
import { OutlinePanel } from "#ui/routes/project/$id/workspace/OutlinePanel.tsx";
import { classes } from "#ui/ui/classes.ts";
import { CommandOptions, useCommand, useCommandFn } from "#ui/commands/manager.ts";
import type { CommandRegistrationId } from "#ui/commands/state.ts";

type CommandPaletteItem = {
	id: string;
	name: string;
	hotkeys?: Array<Hotkey | HotkeySequence>;
};

const groupCommandPaletteItems = (
	regs: Record<CommandRegistrationId, CommandOptions>,
): Array<PickerDialogGroup<CommandPaletteItem>> => {
	const grouped: Map<CommandGroup, Array<CommandPaletteItem>> = new Map();

	for (const [id, cmd] of Object.entries(regs)) {
		if (cmd.enabled === false || cmd.commandPalette === undefined) continue;

		const mitems = grouped.get(cmd.group) ?? [];
		grouped.set(cmd.group, [
			...mitems,
			{
				id,
				name: cmd.commandPalette.label,
				hotkeys: cmd.hotkeys?.map((hk) =>
					"sequence" in hk ? hk.sequence : normalizeRegisterableHotkey(hk.hotkey),
				),
			},
		]);
	}

	return Array.from(grouped.entries())
		.toSorted(Order.mapInput(Order.string, ([g]) => g))
		.map(([group, cmds]) => ({
			value: group,
			items: cmds.toSorted(Order.mapInput(Order.string, (cmd) => cmd.name)),
		}));
};

const CommandPalette: FC<{
	open: boolean;
	onOpenChange: (open: boolean) => void;
}> = ({ open, onOpenChange }) => {
	const regs = useAppSelector((state) => state.commands.registrations);
	const items = groupCommandPaletteItems(regs);
	const getCommandFn = useCommandFn();

	const runCommand = (hotkey: CommandPaletteItem) => {
		onOpenChange(false);
		// oxlint-disable-next-line typescript/no-non-null-assertion: Let it loudly fail.
		getCommandFn(hotkey.id)!("commandPalette");
	};

	return (
		<PickerDialog
			ariaLabel="Command palette"
			closeLabel="Close command palette"
			emptyLabel="No commands found."
			getItemKey={(x) => x.id}
			getItemLabel={(x) => x.name}
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
	const dispatch = useAppDispatch();
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const panelsState = useAppSelector((state) => selectProjectPanelsState(state, projectId));
	const focusedPanel = useFocusedProjectPanel(projectId);
	const openApplyBranchPicker = () => {
		dispatch(projectActions.openApplyBranchPicker({ projectId }));
	};
	const toggleDetails = () => {
		if (focusedPanel === "details" && isPanelVisible(panelsState, "details")) {
			const detailsPanelIndex = panelsState.visiblePanels.indexOf("details");
			const nextPanel = panelsState.visiblePanels[detailsPanelIndex - 1];
			if (nextPanel !== undefined) focusPanel(nextPanel);
		}

		dispatch(projectActions.togglePanel({ projectId, panel: "details" }));
	};

	const applyBranchCommand = useCommand(openApplyBranchPicker, {
		group: "Branches",
		commandPalette: { label: "Apply" },
		shortcutsBar: { label: "Apply" },
		hotkeys: [{ hotkey: "Mod+Shift+A" }],
	});

	const toggleDetailsCommand = useCommand(toggleDetails, {
		group: "Details",
		commandPalette: {
			label: isPanelVisible(panelsState, "details") ? "Close" : "Open",
		},
		shortcutsBar: { label: "Details" },
		hotkeys: [{ hotkey: "D" }],
	});

	return (
		<>
			<ShortcutButton
				className={uiStyles.button}
				hotkeys={applyBranchCommand.hotkeys}
				onClick={applyBranchCommand.commandFn}
			>
				Apply branch
			</ShortcutButton>
			<ShortcutButton
				className={uiStyles.button}
				hotkeys={toggleDetailsCommand.hotkeys}
				aria-pressed={isPanelVisible(panelsState, "details")}
				onClick={toggleDetailsCommand.commandFn}
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
	hotkeyOpts: HotkeyOptions;
}): boolean =>
	hotkeyOpts.ignoreInputs !== false &&
	isInputElement(activeElement) &&
	activeElement !== document.documentElement;

const ShortcutsBar: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const focusedPanel = useFocusedProjectPanel(projectId);
	const activeElement = useActiveElement();
	const regs = useAppSelector((state) => state.commands.registrations);
	const visibleHotkeys = Object.values(regs)
		.flatMap(({ enabled, hotkeys, shortcutsBar }) =>
			enabled !== false && shortcutsBar !== undefined && hotkeys !== undefined
				? hotkeys.flatMap((hk) =>
						// TODO: Render sequences too.
						"sequence" in hk ||
						hk.enabled === false ||
						isInputIgnoredHotkey({ activeElement, hotkeyOpts: hk })
							? []
							: {
									label: shortcutsBar.label,
									hotkey: formatForDisplay(hk.hotkey),
								},
					)
				: [],
		)
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

const usePanelsHotkeys = ({ focusedPanel }: { focusedPanel: PanelType | null }) => {
	useCommand(
		() => {
			focusAdjacentPanel(-1);
		},
		{
			group: "Panels",
			enabled: focusedPanel !== null,
			shortcutsBar: { label: "Focus previous panel" },
			hotkeys: [{ hotkey: "H" }],
		},
	);

	useCommand(
		() => {
			focusAdjacentPanel(1);
		},
		{
			group: "Panels",
			enabled: focusedPanel !== null,
			shortcutsBar: { label: "Focus next panel" },
			hotkeys: [{ hotkey: "L" }],
		},
	);
};

const WorkspacePage: FC = () => {
	const dispatch = useAppDispatch();

	const { id: projectId } = useParams({ from: "/project/$id/workspace" });

	const dialog = useAppSelector((state) => selectProjectDialogState(state, projectId));
	const panelsState = useAppSelector((state) => selectProjectPanelsState(state, projectId));
	const focusedPanel = useFocusedProjectPanel(projectId);

	useCommand(
		() => {
			if (dialog._tag === "CommandPalette") dispatch(projectActions.closeDialog({ projectId }));
			else dispatch(projectActions.openCommandPalette({ projectId, focusedPanel }));
		},
		{
			group: "Global",
			shortcutsBar: { label: "Command palette" },
			hotkeys: [{ hotkey: "Mod+K" }],
		},
	);

	usePanelsHotkeys({ focusedPanel });

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
