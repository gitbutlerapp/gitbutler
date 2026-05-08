import { CommandGroup } from "#ui/commands/groups.ts";
import { useActiveElement } from "#ui/focus.ts";
import { changesSectionOperand, Operand } from "#ui/operands.ts";
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
	(activeElement?.closest("[data-panel]")?.id as Panel | undefined) ?? null;

export const useFocusedProjectPanel = (projectId: string): Panel | null => {
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
	if (currentPanel === null) return;
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
	focusedPanel: Panel | null;
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

	const enterMoveMode = () => {
		dispatch(projectActions.enterMoveMode({ projectId, source: selection }));
		focusPanel("outline");
	};

	const enterRubMode = (source: Operand) => () => {
		dispatch(projectActions.enterRubMode({ projectId, source }));
		focusPanel("outline");
	};

	const enterCutMode = () => {
		dispatch(projectActions.enterCutMode({ projectId, source: selection }));
		focusPanel("outline");
	};

	const enterCommitMode = () => {
		dispatch(projectActions.enterMoveMode({ projectId, source: changesSectionOperand }));
		focusPanel("outline");
	};

	useCommand(selectPreviousItem, {
		enabled: focusedPanel === panel,
		layer: "focused-selection-tree",
		shortcutsBar: { label: "Up" },
		hotkeys: [{ hotkey: "ArrowUp" }, { hotkey: "K" }],
	});

	useCommand(selectNextItem, {
		enabled: focusedPanel === panel,
		layer: "focused-selection-tree",
		shortcutsBar: { label: "Down" },
		hotkeys: [{ hotkey: "ArrowDown" }, { hotkey: "J" }],
	});

	useCommand(selectPreviousSection, {
		enabled: focusedPanel === panel,
		layer: "focused-selection-tree",
		hotkeys: [{ hotkey: "Shift+ArrowUp" }, { hotkey: "Shift+K" }],
	});

	useCommand(selectNextSection, {
		enabled: focusedPanel === panel,
		layer: "focused-selection-tree",
		hotkeys: [{ hotkey: "Shift+ArrowDown" }, { hotkey: "Shift+J" }],
	});

	useCommand(selectFirstItem, {
		enabled: focusedPanel === panel,
		layer: "focused-selection-tree",
		hotkeys: [{ hotkey: "Home" }, { hotkey: "Meta+ArrowUp" }, { sequence: ["G", "G"] }],
	});

	useCommand(selectLastItem, {
		enabled: focusedPanel === panel,
		layer: "focused-selection-tree",
		hotkeys: [{ hotkey: "End" }, { hotkey: "Meta+ArrowDown" }, { hotkey: "Shift+G" }],
	});

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	useCommand(enterMoveMode, {
		enabled: focusedPanel === panel && outlineMode._tag === "Default",
		layer: "focused-selection-tree",
		commandPalette: { group, label: "Move" },
		shortcutsBar: { label: "Move" },
		hotkeys: [{ hotkey: "M" }],
	});

	useCommand(enterCutMode, {
		enabled: focusedPanel === panel && outlineMode._tag === "Default",
		layer: "focused-selection-tree",
		commandPalette: { group, label: "Cut" },
		shortcutsBar: { label: "Cut" },
		hotkeys: [{ hotkey: "Mod+X", ignoreInputs: true }],
	});

	useCommand(enterRubMode(selection), {
		enabled: focusedPanel === panel && outlineMode._tag === "Default",
		layer: "focused-selection-tree",
		commandPalette: { group, label: "Rub" },
		shortcutsBar: { label: "Rub" },
		hotkeys: [{ hotkey: "R" }],
	});

	useCommand(enterRubMode(changesSectionOperand), {
		enabled: focusedPanel === panel && outlineMode._tag === "Default",
		layer: "focused-selection-tree",
		commandPalette: { group, label: "Rub changes" },
		shortcutsBar: { label: "Rub changes" },
		hotkeys: [{ hotkey: "Shift+R" }],
	});

	useCommand(enterCommitMode, {
		enabled: focusedPanel === panel && outlineMode._tag === "Default",
		layer: "focused-selection-tree",
		commandPalette: { group, label: "Commit" },
		shortcutsBar: { label: "Commit" },
		hotkeys: [{ hotkey: "C" }],
	});
};
