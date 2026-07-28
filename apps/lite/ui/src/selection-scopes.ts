import { selectionOperationHotkeys, type CommandGroup } from "#ui/hotkeys.ts";
import type { Placement } from "#ui/operations/operation.ts";
import type { Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { getAdjacent, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { useHotkeySequences, useHotkeys } from "@tanstack/react-hotkeys";

export type SelectionScope = "uncommitted-files" | "outline" | "files" | "diff";
const allSelectionScopes: Set<string> = new Set([
	"uncommitted-files",
	"outline",
	"files",
	"diff",
] satisfies Array<SelectionScope>);

const isSelectionScope = (id: string): id is SelectionScope => allSelectionScopes.has(id);

export const getFocusedSelectionScope = (activeElement: Element | null): SelectionScope | null => {
	const selectionScope = activeElement?.matches("[data-selection-scope]")
		? activeElement.getAttribute("data-selection-scope")
		: undefined;
	if (selectionScope == undefined) return null;
	return isSelectionScope(selectionScope) ? selectionScope : null;
};

export const focusSelectionScope = (selectionScope: SelectionScope): boolean => {
	const el = document.querySelector<HTMLElement>(`[data-selection-scope="${selectionScope}"]`);

	if (el) el.focus({ focusVisible: false });
	return !!el;
};

export const focusHorizontalSelectionScope = ({
	filesVisible,
	offset,
	outlineSelectionScope,
	outlineVisible,
}: {
	filesVisible: boolean;
	offset: -1 | 1;
	outlineSelectionScope: Extract<SelectionScope, "uncommitted-files" | "outline"> | null;
	outlineVisible: boolean;
}) => {
	const currentSelectionScope = getFocusedSelectionScope(document.activeElement);
	const currentOutlineSelectionScope =
		currentSelectionScope === "uncommitted-files" || currentSelectionScope === "outline"
			? currentSelectionScope
			: outlineSelectionScope;

	const orderedSelectionScopes: Array<SelectionScope> = [
		...(outlineVisible ? [currentOutlineSelectionScope ?? "outline"] : []),
		...(filesVisible ? (["files"] satisfies Array<SelectionScope>) : []),
		"diff",
	];

	if (currentSelectionScope === null || !orderedSelectionScopes.includes(currentSelectionScope)) {
		const nextSelectionScope: SelectionScope | undefined =
			offset === 1 ? orderedSelectionScopes.at(0) : orderedSelectionScopes.at(-1);

		if (nextSelectionScope !== undefined) focusSelectionScope(nextSelectionScope);
	} else {
		const nextIndex = orderedSelectionScopes.indexOf(currentSelectionScope) + offset;
		const nextSelectionScope = nextIndex < 0 ? undefined : orderedSelectionScopes.at(nextIndex);
		if (nextSelectionScope !== undefined) focusSelectionScope(nextSelectionScope);
	}
};

export const focusVerticalSelectionScope = (offset: -1 | 1) => {
	const currentSelectionScope = getFocusedSelectionScope(document.activeElement);
	const orderedSelectionScopes: Array<SelectionScope> = ["uncommitted-files", "outline"];
	const currentIndex =
		currentSelectionScope === null ? -1 : orderedSelectionScopes.indexOf(currentSelectionScope);
	if (currentIndex === -1) return;

	const nextIndex = currentIndex + offset;
	const nextSelectionScope = nextIndex < 0 ? undefined : orderedSelectionScopes.at(nextIndex);
	if (nextSelectionScope !== undefined) focusSelectionScope(nextSelectionScope);
};

export const useNavigationIndexHotkeys = <T>({
	navigationIndex,
	projectId,
	group,
	select,
	selection,
	ref,
	selectSectionPredicate,
	operationSourcesForItem,
	getKey,
}: {
	navigationIndex: NavigationIndex<T>;
	projectId: string;
	group: CommandGroup;
	select: (newItem: T) => void;
	selection: T | null;
	ref: React.RefObject<HTMLElement | null>;
	selectSectionPredicate?: (item: T) => boolean;
	/** When omitted, the selection operation hotkeys (move, cut) are not registered. */
	operationSourcesForItem?: (item: T) => Array<Operand>;
	getKey: (item: T) => string;
}) => {
	const dispatch = useAppDispatch();

	const moveSelection = (offset: -1 | 1) => {
		const newItem =
			selection === null
				? navigationIndex.items.at(offset === 1 ? 0 : -1)
				: getAdjacent({ navigationIndex, selection, offset, getKey });
		if (newItem === null || newItem === undefined) return;
		select(newItem);
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
				select(item);
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
		select(newItem);
	};

	const selectLastItem = () => {
		const newItem = navigationIndex.items.at(-1);
		if (newItem === undefined) return;
		select(newItem);
	};

	useHotkeys([
		{
			hotkey: "ArrowUp",
			callback: selectPreviousItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
			},
		},
		{
			hotkey: "K",
			callback: selectPreviousItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
			},
		},
		{
			hotkey: "ArrowDown",
			callback: selectNextItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
			},
		},
		{
			hotkey: "J",
			callback: selectNextItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
			},
		},
		{
			hotkey: "Shift+ArrowUp",
			callback: selectPreviousSection,
			options: {
				conflictBehavior: "allow",
				target: ref,
			},
		},
		{
			hotkey: "Shift+K",
			callback: selectPreviousSection,
			options: {
				conflictBehavior: "allow",
				target: ref,
			},
		},
		{
			hotkey: "Shift+ArrowDown",
			callback: selectNextSection,
			options: {
				conflictBehavior: "allow",
				target: ref,
			},
		},
		{
			hotkey: "Shift+J",
			callback: selectNextSection,
			options: {
				conflictBehavior: "allow",
				target: ref,
			},
		},
		{
			hotkey: "Home",
			callback: selectFirstItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
			},
		},
		{
			hotkey: "Meta+ArrowUp",
			callback: selectFirstItem,
			options: {
				conflictBehavior: "allow",
				ignoreInputs: true,
				target: ref,
			},
		},
		{
			hotkey: "End",
			callback: selectLastItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
			},
		},
		{
			hotkey: "Meta+ArrowDown",
			callback: selectLastItem,
			options: {
				conflictBehavior: "allow",
				ignoreInputs: true,
				target: ref,
			},
		},
		{
			hotkey: "Shift+G",
			callback: selectLastItem,
			options: {
				conflictBehavior: "allow",
				target: ref,
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
			},
		},
	]);

	const enterTransferModeForSelection = (placement: Placement) => {
		if (selection === null || operationSourcesForItem === undefined) return;

		dispatch(
			projectSlice.actions.enterKeyboardTransferMode({
				projectId,
				sources: operationSourcesForItem(selection),
				placement,
			}),
		);

		focusSelectionScope("outline");
	};

	useHotkeys([
		{
			hotkey: selectionOperationHotkeys.move.hotkey,
			callback: () => enterTransferModeForSelection("above"),
			options: {
				conflictBehavior: "allow",
				enabled: selection !== null && operationSourcesForItem !== undefined,
				target: ref,
				meta: { group, name: "Move" },
			},
		},
		{
			hotkey: selectionOperationHotkeys.cut.hotkey,
			callback: () => enterTransferModeForSelection("into"),
			options: {
				conflictBehavior: "allow",
				enabled: selection !== null && operationSourcesForItem !== undefined,
				target: ref,
				ignoreInputs: true,
				meta: { group, name: "Cut" },
			},
		},
	]);
};
