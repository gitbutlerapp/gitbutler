import { useActiveElement } from "#ui/focus.ts";
import { type Operand } from "#ui/operands.ts";
import { selectProjectDialogState } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import {
	getAdjacent,
	getNextSection,
	getPreviousSection,
	NavigationIndex,
} from "#ui/workspace/navigation-index.ts";
import { useHotkeySequences, useHotkeys } from "@tanstack/react-hotkeys";

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
	panel,
	select,
	selection,
}: {
	focusedPanel: Panel | null;
	navigationIndex: NavigationIndex;
	panel: Panel;
	select: (newItem: Operand) => void;
	selection: Operand;
}) => {
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

	const enabled = focusedPanel === panel;
	const conflictBehavior = enabled ? "warn" : "allow";

	useHotkeys([
		{
			hotkey: "ArrowUp",
			callback: selectPreviousItem,
			options: { enabled, conflictBehavior },
		},
		{
			hotkey: "K",
			callback: selectPreviousItem,
			options: { enabled, conflictBehavior },
		},
		{
			hotkey: "ArrowDown",
			callback: selectNextItem,
			options: { enabled, conflictBehavior },
		},
		{
			hotkey: "J",
			callback: selectNextItem,
			options: { enabled, conflictBehavior },
		},
		{
			hotkey: "Shift+ArrowUp",
			callback: selectPreviousSection,
			options: { enabled, conflictBehavior },
		},
		{
			hotkey: "Shift+K",
			callback: selectPreviousSection,
			options: { enabled, conflictBehavior },
		},
		{
			hotkey: "Shift+ArrowDown",
			callback: selectNextSection,
			options: { enabled, conflictBehavior },
		},
		{
			hotkey: "Shift+J",
			callback: selectNextSection,
			options: { enabled, conflictBehavior },
		},
		{
			hotkey: "Home",
			callback: selectFirstItem,
			options: { enabled, conflictBehavior },
		},
		{
			hotkey: "Meta+ArrowUp",
			callback: selectFirstItem,
			options: { enabled, conflictBehavior },
		},
		{
			hotkey: "End",
			callback: selectLastItem,
			options: { enabled, conflictBehavior },
		},
		{
			hotkey: "Meta+ArrowDown",
			callback: selectLastItem,
			options: { enabled, conflictBehavior },
		},
		{
			hotkey: "Shift+G",
			callback: selectLastItem,
			options: { enabled, conflictBehavior },
		},
	]);

	useHotkeySequences([
		{
			sequence: ["G", "G"],
			callback: selectFirstItem,
			options: { enabled, conflictBehavior },
		},
	]);
};
