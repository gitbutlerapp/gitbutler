import type { CommandGroup } from "#ui/hotkeys.ts";
import { type OperationType } from "#ui/operations/operation.ts";
import { keyboardTransferOperationMode } from "#ui/outline/mode.ts";
import { operandIdentityKey, type Operand } from "#ui/operands.ts";
import {
	projectActions,
	selectProjectOutlineModeState,
	selectProjectSelectionFiles,
	selectProjectSelectionOutline,
} from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import {
	getAdjacent,
	navigationIndexIncludes,
	type NavigationIndex,
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

export const resolveNavigationIndexSelection = (
	navigationIndex: NavigationIndex,
	selection: Operand | null,
): Operand | null =>
	selection && navigationIndexIncludes(navigationIndex, selection)
		? selection
		: (navigationIndex.items[0] ?? null);

export const useFilesSelection = (projectId: string, navigationIndex: NavigationIndex) => {
	const selection = useAppSelector((state) => selectProjectSelectionFiles(state, projectId));
	return resolveNavigationIndexSelection(navigationIndex, selection);
};

export const useOutlineSelection = ({
	projectId,
	navigationIndex,
}: {
	projectId: string;
	navigationIndex: NavigationIndex;
}) => {
	const selectionState = useAppSelector((state) => selectProjectSelectionOutline(state, projectId));
	return resolveNavigationIndexSelection(navigationIndex, selectionState);
};

export const useNavigationIndexHotkeys = ({
	navigationIndex,
	projectId,
	group,
	selectionScope,
	select,
	selection,
	ref,
	selectSectionPredicate,
}: {
	navigationIndex: NavigationIndex;
	projectId: string;
	group: CommandGroup;
	selectionScope: SelectionScope;
	select: (newItem: Operand) => void;
	selection: Operand | null;
	ref: React.RefObject<HTMLElement | null>;
	selectSectionPredicate?: (operand: Operand) => boolean;
}) => {
	const dispatch = useAppDispatch();

	const selectAndFocus = (newItem: Operand) => {
		select(newItem);
		focusSelectionScope(selectionScope);
	};

	const moveSelection = (offset: -1 | 1) => {
		const newItem =
			selection === null
				? navigationIndex.items.at(offset === 1 ? 0 : -1)
				: getAdjacent({ navigationIndex, selection, offset });
		if (!newItem) return;
		selectAndFocus(newItem);
	};

	const selectPreviousItem = () => {
		moveSelection(-1);
	};

	const selectNextItem = () => {
		moveSelection(1);
	};

	const moveToMatchingItem = (offset: -1 | 1, predicate: (operand: Operand) => boolean) => {
		if (!selection) return;

		const selectionIndex = navigationIndex.indexByKey.get(operandIdentityKey(selection));
		if (selectionIndex === undefined) return;

		const currentItem = navigationIndex.items[selectionIndex];
		const startsOnMatch = currentItem !== undefined && predicate(currentItem);
		let itemIndex = selectionIndex + (offset === -1 && !startsOnMatch ? 0 : offset);

		while (itemIndex >= 0 && itemIndex < navigationIndex.items.length) {
			const item = navigationIndex.items[itemIndex];
			if (item && predicate(item)) {
				selectAndFocus(item);
				return;
			}
			itemIndex += offset;
		}
	};

	const selectNextSection = () => {
		if (!selectSectionPredicate) return;
		moveToMatchingItem(1, selectSectionPredicate);
	};

	const selectPreviousSection = () => {
		if (!selectSectionPredicate) return;
		moveToMatchingItem(-1, selectSectionPredicate);
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

	const operationEnabled = outlineMode._tag === "Default" && selection !== null;

	const enterTransferModeForSelection = (operationType: OperationType) => {
		if (!selection) return;

		enterTransferMode(selection, operationType);
	};

	useHotkeys([
		{
			hotkey: "M",
			callback: () => enterTransferModeForSelection("moveAbove"),
			options: {
				conflictBehavior: "allow",
				enabled: operationEnabled,
				target: ref,
				meta: { group, name: "Move" },
			},
		},
		{
			hotkey: "Mod+X",
			callback: () => enterTransferModeForSelection("rub"),
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
			callback: () => enterTransferModeForSelection("rub"),
			options: {
				conflictBehavior: "allow",
				enabled: operationEnabled,
				target: ref,
				meta: { group, name: "Cut" },
			},
		},
	]);
};
