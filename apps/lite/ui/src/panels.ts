import type { CommandGroup } from "#ui/hotkeys.ts";
import { type OperationType } from "#ui/operations/operation.ts";
import { keyboardTransferOperationMode } from "#ui/outline/mode.ts";
import { type Operand } from "#ui/operands.ts";
import { projectActions, selectProjectOutlineModeState } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import {
	getAdjacent,
	getNextSection,
	getPreviousSection,
	NavigationIndex,
} from "#ui/workspace/navigation-index.ts";
import { useHotkeySequences, useHotkeys } from "@tanstack/react-hotkeys";
import type { PanelsState } from "./panels/state.ts";

export type Panel = "outline" | "files" | "details";
const allPanels: Array<Panel> = ["outline", "files", "details"];

const isProjectPanel = (id: string): id is Panel => allPanels.includes(id as Panel);

export const getFocusedProjectPanel = (activeElement: Element | null): Panel | null => {
	const panelId = activeElement?.matches("[data-panel]") ? activeElement.id : undefined;
	if (panelId === undefined) return null;
	return isProjectPanel(panelId) ? panelId : null;
};

export const focusPanel = (panel: Panel) => {
	document.getElementById(panel)?.focus({ focusVisible: false });
};

export const focusAdjacentPanel = (panelsState: PanelsState, offset: -1 | 1) => {
	const currentPanel = getFocusedProjectPanel(document.activeElement);

	const orderedPanels: Array<Panel> = [
		"outline",
		...(panelsState.filesVisible ? (["files"] satisfies Array<Panel>) : []),
		"details",
	];

	if (currentPanel === null) {
		const nextPanel: Panel | undefined = offset === 1 ? orderedPanels.at(0) : orderedPanels.at(-1);

		if (nextPanel !== undefined) focusPanel(nextPanel);
	} else {
		const curr = orderedPanels.indexOf(currentPanel);
		// oxlint-disable-next-line typescript/no-non-null-assertion: This shouldn't ever fail.
		const nextPanel = orderedPanels.at((curr + offset) % orderedPanels.length)!;

		focusPanel(nextPanel);
	}
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

	const navigationEnabled = focusedPanel === panel;
	useHotkeys([
		{
			hotkey: "ArrowUp",
			callback: selectPreviousItem,
			options: {
				conflictBehavior: "allow",
				enabled: navigationEnabled,
				meta: { group, name: "Select previous" },
			},
		},
		{
			hotkey: "K",
			callback: selectPreviousItem,
			options: {
				conflictBehavior: "allow",
				enabled: navigationEnabled,
				meta: { group, name: "Select previous" },
			},
		},
		{
			hotkey: "ArrowDown",
			callback: selectNextItem,
			options: {
				conflictBehavior: "allow",
				enabled: navigationEnabled,
				meta: { group, name: "Select next" },
			},
		},
		{
			hotkey: "J",
			callback: selectNextItem,
			options: {
				conflictBehavior: "allow",
				enabled: navigationEnabled,
				meta: { group, name: "Select next" },
			},
		},
		{
			hotkey: "Shift+ArrowUp",
			callback: selectPreviousSection,
			options: {
				conflictBehavior: "allow",
				enabled: navigationEnabled,
				meta: { group, name: "Select previous section" },
			},
		},
		{
			hotkey: "Shift+K",
			callback: selectPreviousSection,
			options: {
				conflictBehavior: "allow",
				enabled: navigationEnabled,
				meta: { group, name: "Select previous section" },
			},
		},
		{
			hotkey: "Shift+ArrowDown",
			callback: selectNextSection,
			options: {
				conflictBehavior: "allow",
				enabled: navigationEnabled,
				meta: { group, name: "Select next section" },
			},
		},
		{
			hotkey: "Shift+J",
			callback: selectNextSection,
			options: {
				conflictBehavior: "allow",
				enabled: navigationEnabled,
				meta: { group, name: "Select next section" },
			},
		},
		{
			hotkey: "Home",
			callback: selectFirstItem,
			options: {
				conflictBehavior: "allow",
				enabled: navigationEnabled,
				meta: { group, name: "Select first" },
			},
		},
		{
			hotkey: "Meta+ArrowUp",
			callback: selectFirstItem,
			options: {
				conflictBehavior: "allow",
				enabled: navigationEnabled,
				meta: { group, name: "Select first" },
			},
		},
		{
			hotkey: "End",
			callback: selectLastItem,
			options: {
				conflictBehavior: "allow",
				enabled: navigationEnabled,
				meta: { group, name: "Select last" },
			},
		},
		{
			hotkey: "Meta+ArrowDown",
			callback: selectLastItem,
			options: {
				conflictBehavior: "allow",
				enabled: navigationEnabled,
				meta: { group, name: "Select last" },
			},
		},
		{
			hotkey: "Shift+G",
			callback: selectLastItem,
			options: {
				conflictBehavior: "allow",
				enabled: navigationEnabled,
				meta: { group, name: "Select last" },
			},
		},
	]);

	useHotkeySequences([
		{
			sequence: ["G", "G"],
			callback: selectFirstItem,
			options: {
				conflictBehavior: "allow",
				enabled: navigationEnabled,
				meta: { group, name: "Select first" },
			},
		},
	]);

	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const enterTransferMode = (source: Operand, operationType: OperationType) => {
		dispatch(
			projectActions.enterTransferMode({
				projectId,
				mode: keyboardTransferOperationMode({
					source,
					operationType,
				}),
			}),
		);
		focusPanel("outline");
	};

	const operationEnabled = focusedPanel === panel && outlineMode._tag === "Default";
	useHotkeys([
		{
			hotkey: "M",
			callback: () => enterTransferMode(selection, "moveAbove"),
			options: {
				conflictBehavior: "allow",
				enabled: operationEnabled,
				meta: { group, name: "Move" },
			},
		},
		{
			hotkey: "Mod+X",
			callback: () => enterTransferMode(selection, "rub"),
			options: {
				conflictBehavior: "allow",
				enabled: operationEnabled,
				ignoreInputs: true,
				meta: { group, name: "Cut" },
			},
		},
		{
			hotkey: "R",
			callback: () => enterTransferMode(selection, "rub"),
			options: {
				conflictBehavior: "allow",
				enabled: operationEnabled,
				meta: { group, name: "Cut" },
			},
		},
	]);
};
