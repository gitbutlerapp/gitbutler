import { CommandGroup } from "#ui/commands/groups.ts";
import { useActiveElement } from "#ui/focus.ts";
import { changesSectionOperand, Operand } from "#ui/operands.ts";
import {
	projectActions,
	selectProjectOutlineModeState,
	selectProjectPickerDialogState,
} from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import {
	getAdjacent,
	getNextSection,
	getPreviousSection,
	NavigationIndex,
} from "#ui/workspace/navigation-index.ts";
import { useHotkeys, useHotkeySequence } from "@tanstack/react-hotkeys";

export type Panel = "outline" | "files" | "details";
export const allPanels: Array<Panel> = ["outline", "files", "details"];

const getFocusedProjectPanel = (activeElement: Element | null) =>
	(activeElement?.closest("[data-panel]")?.id as Panel | undefined) ?? null;

export const useFocusedProjectPanel = (projectId: string): Panel | null => {
	const activeElement = useActiveElement();
	const focusedPanel = getFocusedProjectPanel(activeElement);
	const pickerDialog = useAppSelector((state) => selectProjectPickerDialogState(state, projectId));
	return pickerDialog._tag === "CommandPalette" ? pickerDialog.focusedPanel : focusedPanel;
};

export const focusPanel = (panel: Panel) => {
	document.getElementById(panel)?.focus({ focusVisible: false });
};

export const useNavigationIndexHotkeys = ({
	focusedPanel,
	navigationIndex,
	projectId,
	group,
	panel,
	focusPanel,
	select,
	selection,
}: {
	focusedPanel: Panel | null;
	navigationIndex: NavigationIndex;
	projectId: string;
	group: CommandGroup;
	panel: Panel;
	focusPanel: (panel: Panel) => void;
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

	const enterRubMode = () => {
		dispatch(projectActions.enterRubMode({ projectId, source: selection }));
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

	useHotkeys(
		[
			{
				hotkey: "ArrowUp",
				callback: selectPreviousItem,
				options: { meta: { group, name: "Up", commandPalette: false } },
			},
			{
				hotkey: "K",
				callback: selectPreviousItem,
				// Hidden until we can combine in shortcuts bar.
				options: { meta: { group, shortcutsBar: false } },
			},
			{
				hotkey: "ArrowDown",
				callback: selectNextItem,
				options: { meta: { group, name: "Down", commandPalette: false } },
			},
			{
				hotkey: "J",
				callback: selectNextItem,
				// Hidden until we can combine in shortcuts bar.
				options: { meta: { group, shortcutsBar: false } },
			},
			{
				hotkey: "Shift+ArrowUp",
				callback: selectPreviousSection,
				options: {
					meta: {
						group,
						name: "Previous section",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "Shift+K",
				callback: selectPreviousSection,
				options: {
					meta: {
						group,
						name: "Previous section",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "Shift+ArrowDown",
				callback: selectNextSection,
				options: {
					meta: {
						group,
						name: "Next section",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "Shift+J",
				callback: selectNextSection,
				options: {
					meta: {
						group,
						name: "Next section",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "Home",
				callback: selectFirstItem,
				options: {
					meta: {
						group,
						name: "First item",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "Meta+ArrowUp",
				callback: selectFirstItem,
				options: {
					meta: {
						group,
						name: "First item",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "End",
				callback: selectLastItem,
				options: {
					meta: {
						group,
						name: "Last item",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "Meta+ArrowDown",
				callback: selectLastItem,
				options: {
					meta: {
						group,
						name: "Last item",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
			{
				hotkey: "Shift+G",
				callback: selectLastItem,
				options: {
					meta: {
						group,
						name: "Last item",
						commandPalette: false,
						shortcutsBar: false,
					},
				},
			},
		],
		{
			enabled: focusedPanel === panel,
			conflictBehavior: "allow",
		},
	);

	useHotkeySequence(["G", "G"], selectFirstItem, {
		enabled: focusedPanel === panel,
		conflictBehavior: "allow",
		meta: {
			group,
			name: "First item",
			commandPalette: false,
			shortcutsBar: false,
		},
	});

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	useHotkeys(
		[
			{
				hotkey: "M",
				callback: enterMoveMode,
				options: { meta: { group, name: "Move" } },
			},
			{
				hotkey: "Mod+X",
				callback: enterCutMode,
				options: {
					ignoreInputs: true,
					meta: { group, name: "Cut" },
				},
			},
			{
				hotkey: "R",
				callback: enterRubMode,
				options: { meta: { group, name: "Rub" } },
			},
			{
				hotkey: "C",
				callback: enterCommitMode,
				options: { meta: { group, name: "Commit" } },
			},
		],
		{
			enabled: focusedPanel === panel && outlineMode._tag === "Default",
			conflictBehavior: "allow",
		},
	);
};
