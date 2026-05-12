import { CommandGroup } from "#ui/commands/groups.ts";
import { useActiveElement } from "#ui/focus.ts";
import { type OperationType } from "#ui/operations/operation.ts";
import { changesSectionOperand, type Operand } from "#ui/operands.ts";
import {
	projectActions,
	selectProjectOutlineModeState,
	selectProjectDialogState,
} from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import {
	getAdjacent,
	getNextSection,
	getPreviousSection,
	NavigationIndex,
} from "#ui/workspace/navigation-index.ts";
import { useCommand } from "#ui/commands/manager.ts";

export type Panel = "outline" | "files" | "details";
export const orderedPanels: Array<Panel> = ["outline", "files", "details"];

const getFocusedProjectPanel = (activeElement: Element | null) =>
	activeElement?.closest("[data-panel]")?.id as Panel | undefined;

export const useFocusedProjectPanel = (projectId: string): Panel | undefined => {
	const activeElement = useActiveElement();
	const focusedPanel = getFocusedProjectPanel(activeElement);
	const dialog = useAppSelector((state) => selectProjectDialogState(state, projectId));
	return dialog._tag === "CommandPalette" ? dialog.focusedPanel : focusedPanel;
};

export const focusPanel = (panel: Panel) => {
	document.getElementById(panel)?.focus({ focusVisible: false });
};

export const focusAdjacentPanel = (offset: -1 | 1) => {
	const currentPanel = getFocusedProjectPanel(document.activeElement);
	if (currentPanel === undefined) return;
	const nextPanel = orderedPanels[orderedPanels.indexOf(currentPanel) + offset];
	if (nextPanel === undefined) return;
	focusPanel(nextPanel);
};

export const useNavigationIndexHotkeys = ({
	focusedPanel,
	navigationIndex,
	projectId,
	group,
	panel,
	select,
	selection,
}: {
	focusedPanel: Panel | undefined;
	navigationIndex: NavigationIndex;
	projectId: string;
	group: CommandGroup;
	panel: Panel;
	select: (newItem: Operand) => void;
	selection: Operand;
}) => {
	const dispatch = useAppDispatch();

	const selectAndFocus = (newItem: Operand) => {
		select(newItem);
		focusPanel(panel);
	};

	const moveSelection = (offset: -1 | 1) => {
		const newItem = getAdjacent({ navigationIndex, selection, offset });
		if (!newItem) return;
		selectAndFocus(newItem);
	};

	const selectPreviousItem = () => {
		moveSelection(-1);
	};

	const selectNextItem = () => {
		moveSelection(1);
	};

	const selectNextSection = () => {
		const newItem = getNextSection({ navigationIndex, selection });
		if (!newItem) return;
		selectAndFocus(newItem);
	};

	const selectPreviousSection = () => {
		const newItem = getPreviousSection({ navigationIndex, selection });
		if (!newItem) return;
		selectAndFocus(newItem);
	};

	const selectFirstItem = () => {
		const newItem = navigationIndex.items[0];
		if (!newItem) return;
		selectAndFocus(newItem);
	};

	const selectLastItem = () => {
		const newItem = navigationIndex.items.at(-1);
		if (!newItem) return;
		selectAndFocus(newItem);
	};

	useCommand(selectPreviousItem, {
		group,
		enabled: focusedPanel === panel,
		shortcutsBar: { label: "Up" },
		hotkeys: [{ hotkey: "ArrowUp" }, { hotkey: "K" }],
	});

	useCommand(selectNextItem, {
		group,
		enabled: focusedPanel === panel,
		shortcutsBar: { label: "Down" },
		hotkeys: [{ hotkey: "ArrowDown" }, { hotkey: "J" }],
	});

	useCommand(selectPreviousSection, {
		group,
		enabled: focusedPanel === panel,
		hotkeys: [{ hotkey: "Shift+ArrowUp" }, { hotkey: "Shift+K" }],
	});

	useCommand(selectNextSection, {
		group,
		enabled: focusedPanel === panel,
		hotkeys: [{ hotkey: "Shift+ArrowDown" }, { hotkey: "Shift+J" }],
	});

	useCommand(selectFirstItem, {
		group,
		enabled: focusedPanel === panel,
		hotkeys: [{ hotkey: "Home" }, { hotkey: "Meta+ArrowUp" }, { sequence: ["G", "G"] }],
	});

	useCommand(selectLastItem, {
		group,
		enabled: focusedPanel === panel,
		hotkeys: [{ hotkey: "End" }, { hotkey: "Meta+ArrowDown" }, { hotkey: "Shift+G" }],
	});

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const enterCutMode = (source: Operand, operationType: OperationType) => {
		dispatch(projectActions.enterCutMode({ projectId, source, operationType }));
		focusPanel("outline");
	};

	useCommand(() => enterCutMode(selection, "moveAbove"), {
		group,
		enabled: focusedPanel === panel && outlineMode._tag === "Default",
		commandPalette: { label: "Move" },
		shortcutsBar: { label: "Move" },
		hotkeys: [{ hotkey: "M" }],
	});

	useCommand(() => enterCutMode(selection, "rub"), {
		group,
		enabled: focusedPanel === panel && outlineMode._tag === "Default",
		commandPalette: { label: "Cut" },
		shortcutsBar: { label: "Cut" },
		hotkeys: [{ hotkey: "Mod+X", ignoreInputs: true }, { hotkey: "R" }],
	});

	useCommand(() => enterCutMode(changesSectionOperand, "moveAbove"), {
		group,
		enabled: focusedPanel === panel && outlineMode._tag === "Default",
		commandPalette: { label: "Commit" },
		shortcutsBar: { label: "Commit" },
		hotkeys: [{ hotkey: "C" }],
	});
};
