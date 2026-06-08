import { selectionOperationHotkeys, type CommandGroup } from "#ui/hotkeys.ts";
import { type OperationType } from "#ui/operations/operation.ts";
import { keyboardTransferOperationMode } from "#ui/outline/mode.ts";
import { fileOperand, operandIdentityKey, type FileOperand, type Operand } from "#ui/operands.ts";
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

export const focusAdjacentSelectionScope = ({
	filesVisible,
	offset,
	outlineVisible,
}: {
	filesVisible: boolean;
	offset: -1 | 1;
	outlineVisible: boolean;
}) => {
	const currentSelectionScope = getFocusedSelectionScope(document.activeElement);

	const orderedSelectionScopes: Array<SelectionScope> = [
		...(outlineVisible ? (["outline"] satisfies Array<SelectionScope>) : []),
		...(filesVisible ? (["files"] satisfies Array<SelectionScope>) : []),
		"diff",
	];

	if (currentSelectionScope === null || !orderedSelectionScopes.includes(currentSelectionScope)) {
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

export const resolveNavigationIndexSelection = <T>(
	navigationIndex: NavigationIndex<T>,
	selection: T | null,
	getKey: (item: T) => string,
): T | null =>
	selection !== null && navigationIndexIncludes(navigationIndex, selection, getKey)
		? selection
		: (navigationIndex.items[0] ?? null);

const fileOperandIdentityKey = (operand: FileOperand): string =>
	operandIdentityKey(fileOperand(operand));

export const useFilesSelection = (
	projectId: string,
	navigationIndex: NavigationIndex<FileOperand>,
) => {
	const selection = useAppSelector((state) => selectProjectSelectionFiles(state, projectId));
	return resolveNavigationIndexSelection(navigationIndex, selection, fileOperandIdentityKey);
};

export const useOutlineSelection = ({
	projectId,
	navigationIndex,
}: {
	projectId: string;
	navigationIndex: NavigationIndex<Operand>;
}) => {
	const selectionState = useAppSelector((state) => selectProjectSelectionOutline(state, projectId));
	return resolveNavigationIndexSelection(navigationIndex, selectionState, operandIdentityKey);
};

export const useNavigationIndexHotkeys = <T>({
	navigationIndex,
	projectId,
	group,
	selectionScope,
	select,
	selection,
	ref,
	selectSectionPredicate,
	operationSourceForItem,
	getKey,
}: {
	navigationIndex: NavigationIndex<T>;
	projectId: string;
	group: CommandGroup;
	selectionScope: SelectionScope;
	select: (newItem: T) => void;
	selection: T | null;
	ref: React.RefObject<HTMLElement | null>;
	selectSectionPredicate?: (item: T) => boolean;
	operationSourceForItem: (item: T) => Operand;
	getKey: (item: T) => string;
}) => {
	const dispatch = useAppDispatch();

	const selectAndFocus = (newItem: T) => {
		select(newItem);
		focusSelectionScope(selectionScope);
	};

	const moveSelection = (offset: -1 | 1) => {
		const newItem =
			selection === null
				? navigationIndex.items.at(offset === 1 ? 0 : -1)
				: getAdjacent({ navigationIndex, selection, offset, getKey });
		if (newItem === null || newItem === undefined) return;
		selectAndFocus(newItem);
	};

	const selectPreviousItem = () => {
		moveSelection(-1);
	};

	const selectNextItem = () => {
		moveSelection(1);
	};

	const moveToMatchingItem = (offset: -1 | 1, predicate: (item: T) => boolean) => {
		if (selection === null) return;

		const selectionIndex = navigationIndex.indexByKey.get(getKey(selection));
		if (selectionIndex === undefined) return;

		const currentItem = navigationIndex.items[selectionIndex];
		const startsOnMatch = currentItem !== undefined && predicate(currentItem);
		let itemIndex = selectionIndex + (offset === -1 && !startsOnMatch ? 0 : offset);

		while (itemIndex >= 0 && itemIndex < navigationIndex.items.length) {
			const item = navigationIndex.items[itemIndex];
			if (item !== undefined && predicate(item)) {
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
		if (newItem === undefined) return;
		selectAndFocus(newItem);
	};

	const selectLastItem = () => {
		const newItem = navigationIndex.items.at(-1);
		if (newItem === undefined) return;
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
		if (selection === null) return;

		enterTransferMode(operationSourceForItem(selection), operationType);
	};

	useHotkeys([
		{
			hotkey: selectionOperationHotkeys.move.hotkey,
			callback: () => enterTransferModeForSelection("moveAbove"),
			options: {
				conflictBehavior: "allow",
				enabled: operationEnabled,
				target: ref,
				meta: { ...selectionOperationHotkeys.move.meta, group },
			},
		},
		{
			hotkey: selectionOperationHotkeys.cut.hotkey,
			callback: () => enterTransferModeForSelection("squash"),
			options: {
				conflictBehavior: "allow",
				enabled: operationEnabled,
				target: ref,
				ignoreInputs: true,
				meta: { ...selectionOperationHotkeys.cut.meta, group },
			},
		},
		{
			hotkey: selectionOperationHotkeys.squash.hotkey,
			callback: () => enterTransferModeForSelection("squash"),
			options: {
				conflictBehavior: "allow",
				enabled: operationEnabled,
				target: ref,
				meta: { ...selectionOperationHotkeys.squash.meta, group },
			},
		},
	]);
};
