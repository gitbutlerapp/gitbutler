import { useActiveElement } from "#ui/focus.ts";
import type { CommandGroup } from "#ui/commands/groups.ts";
import { Operand } from "#ui/operands.ts";
import { selectProjectDialogState } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import {
	getAdjacent,
	getNextSection,
	getPreviousSection,
	NavigationIndex,
} from "#ui/workspace/navigation-index.ts";
import { useHotkey, useHotkeySequence } from "@tanstack/react-hotkeys";

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
	group,
	panel,
	select,
	selection,
}: {
	focusedPanel: Panel | null;
	navigationIndex: NavigationIndex;
	group: CommandGroup;
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

	useHotkey("ArrowUp", selectPreviousItem, {
		conflictBehavior: "allow",
		enabled: focusedPanel === panel,
		meta: {
			group,
			name: "Move up",
			shortcutsBar: false,
		},
	});

	useHotkey("ArrowDown", selectNextItem, {
		conflictBehavior: "allow",
		enabled: focusedPanel === panel,
		meta: {
			group,
			name: "Move down",
			shortcutsBar: false,
		},
	});

	useHotkey("Shift+ArrowUp", selectPreviousSection, {
		conflictBehavior: "allow",
		enabled: focusedPanel === panel,
		meta: {
			group,
			name: "Move to previous section",
			shortcutsBar: false,
		},
	});

	useHotkey("Shift+K", selectPreviousSection, {
		conflictBehavior: "allow",
		enabled: focusedPanel === panel,
		meta: {
			group,
			name: "Move to previous section",
			shortcutsBar: false,
		},
	});

	useHotkey("Shift+ArrowDown", selectNextSection, {
		conflictBehavior: "allow",
		enabled: focusedPanel === panel,
		meta: {
			group,
			name: "Move to next section",
			shortcutsBar: false,
		},
	});

	useHotkey("Shift+J", selectNextSection, {
		conflictBehavior: "allow",
		enabled: focusedPanel === panel,
		meta: {
			group,
			name: "Move to next section",
			shortcutsBar: false,
		},
	});

	useHotkey("Home", selectFirstItem, {
		conflictBehavior: "allow",
		enabled: focusedPanel === panel,
		meta: {
			group,
			name: "Move to first item",
			shortcutsBar: false,
		},
	});

	useHotkey("Meta+ArrowUp", selectFirstItem, {
		conflictBehavior: "allow",
		enabled: focusedPanel === panel,
		meta: {
			group,
			name: "Move to first item",
			shortcutsBar: false,
		},
	});

	useHotkeySequence(["G", "G"], selectFirstItem, {
		conflictBehavior: "allow",
		enabled: focusedPanel === panel,
		meta: {
			group,
			name: "Move to first item",
			shortcutsBar: false,
		},
	});

	useHotkey("End", selectLastItem, {
		conflictBehavior: "allow",
		enabled: focusedPanel === panel,
		meta: {
			group,
			name: "Move to last item",
			shortcutsBar: false,
		},
	});

	useHotkey("Meta+ArrowDown", selectLastItem, {
		conflictBehavior: "allow",
		enabled: focusedPanel === panel,
		meta: {
			group,
			name: "Move to last item",
			shortcutsBar: false,
		},
	});

	useHotkey("Shift+G", selectLastItem, {
		conflictBehavior: "allow",
		enabled: focusedPanel === panel,
		meta: {
			group,
			name: "Move to last item",
			shortcutsBar: false,
		},
	});
};
