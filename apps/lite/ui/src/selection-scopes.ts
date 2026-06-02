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

export type SelectionScope = "outline" | "files" | "diff";
const allSelectionScopes: Array<SelectionScope> = ["outline", "files", "diff"];

const isSelectionScope = (id: string): id is SelectionScope =>
	allSelectionScopes.includes(id as SelectionScope);

export const getFocusedSelectionScope = (activeElement: Element | null): SelectionScope | null => {
	const selectionScopeId = activeElement?.matches("[data-selection-scope]")
		? activeElement.id
		: undefined;
	if (selectionScopeId === undefined) return null;
	return isSelectionScope(selectionScopeId) ? selectionScopeId : null;
};

export const focusSelectionScope = (selectionScope: SelectionScope) => {
	document.getElementById(selectionScope)?.focus({ focusVisible: false });
};

export const focusAdjacentSelectionScope = (filesVisible: boolean, offset: -1 | 1) => {
	const currentSelectionScope = getFocusedSelectionScope(document.activeElement);

	const orderedSelectionScopes: Array<SelectionScope> = [
		"outline",
		...(filesVisible ? (["files"] satisfies Array<SelectionScope>) : []),
		"diff",
	];

	if (currentSelectionScope === null) {
		const nextSelectionScope: SelectionScope | undefined =
			offset === 1 ? orderedSelectionScopes.at(0) : orderedSelectionScopes.at(-1);

		if (nextSelectionScope !== undefined) focusSelectionScope(nextSelectionScope);
	} else {
		const curr = orderedSelectionScopes.indexOf(currentSelectionScope);
		// oxlint-disable-next-line typescript/no-non-null-assertion: This shouldn't ever fail.
		const nextSelectionScope = orderedSelectionScopes.at(
			(curr + offset) % orderedSelectionScopes.length,
		)!;

		focusSelectionScope(nextSelectionScope);
	}
};

export const useNavigationIndexHotkeys = ({
	navigationIndex,
	projectId,
	group,
	selectionScope,
	select,
	selection,
	ref,
}: {
	navigationIndex: NavigationIndex;
	projectId: string;
	group: CommandGroup;
	selectionScope: SelectionScope;
	select: (newItem: Operand) => void;
	selection: Operand;
	ref: React.RefObject<HTMLElement | null>;
}) => {
	const dispatch = useAppDispatch();

	const selectAndFocus = (newItem: Operand) => {
		select(newItem);
		focusSelectionScope(selectionScope);
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

	useHotkeys([
		{
			hotkey: "ArrowUp",
			callback: selectPreviousItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
				meta: { group, name: "Select previous" },
			},
		},
		{
			hotkey: "K",
			callback: selectPreviousItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
				meta: { group, name: "Select previous" },
			},
		},
		{
			hotkey: "ArrowDown",
			callback: selectNextItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
				meta: { group, name: "Select next" },
			},
		},
		{
			hotkey: "J",
			callback: selectNextItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
				meta: { group, name: "Select next" },
			},
		},
		{
			hotkey: "Shift+ArrowUp",
			callback: selectPreviousSection,
			options: {
				conflictBehavior: "allow",
				target: ref,
				meta: { group, name: "Select previous section" },
			},
		},
		{
			hotkey: "Shift+K",
			callback: selectPreviousSection,
			options: {
				conflictBehavior: "allow",
				target: ref,
				meta: { group, name: "Select previous section" },
			},
		},
		{
			hotkey: "Shift+ArrowDown",
			callback: selectNextSection,
			options: {
				conflictBehavior: "allow",
				target: ref,
				meta: { group, name: "Select next section" },
			},
		},
		{
			hotkey: "Shift+J",
			callback: selectNextSection,
			options: {
				conflictBehavior: "allow",
				target: ref,
				meta: { group, name: "Select next section" },
			},
		},
		{
			hotkey: "Home",
			callback: selectFirstItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
				meta: { group, name: "Select first" },
			},
		},
		{
			hotkey: "Meta+ArrowUp",
			callback: selectFirstItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
				meta: { group, name: "Select first" },
			},
		},
		{
			hotkey: "End",
			callback: selectLastItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
				meta: { group, name: "Select last" },
			},
		},
		{
			hotkey: "Meta+ArrowDown",
			callback: selectLastItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
				meta: { group, name: "Select last" },
			},
		},
		{
			hotkey: "Shift+G",
			callback: selectLastItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
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
				target: ref,
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
		focusSelectionScope("outline");
	};

	const operationEnabled = outlineMode._tag === "Default";
	useHotkeys([
		{
			hotkey: "M",
			callback: () => enterTransferMode(selection, "moveAbove"),
			options: {
				conflictBehavior: "allow",
				enabled: operationEnabled,
				target: ref,
				meta: { group, name: "Move" },
			},
		},
		{
			hotkey: "Mod+X",
			callback: () => enterTransferMode(selection, "rub"),
			options: {
				conflictBehavior: "allow",
				enabled: operationEnabled,
				target: ref,
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
				target: ref,
				meta: { group, name: "Cut" },
			},
		},
	]);
};
