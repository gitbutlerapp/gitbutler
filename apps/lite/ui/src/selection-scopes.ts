import { selectionOperationHotkeys, type CommandGroup } from "#ui/hotkeys.ts";
import type { Placement } from "#ui/operations/operation.ts";
import type { Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { getAdjacent, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { useHotkeySequences, useHotkeys } from "@tanstack/react-hotkeys";
import { useRef } from "react";

export type SelectionScope = "details" | "uncommitted-files" | "outline" | "files" | "diff" | "pr";
const allSelectionScopes: Set<string> = new Set([
	"details",
	"uncommitted-files",
	"outline",
	"files",
	"diff",
	"pr",
] satisfies Array<SelectionScope>);

// Supports arbitrarily nested scopes. Must share that ancestral relationship in the DOM. Tries from
// left to right.
const selectionScopeChildren: Partial<Record<SelectionScope, Set<SelectionScope>>> = {
	details: new Set(["diff", "pr", "files"]),
};

const isSelectionScope = (id: string): id is SelectionScope => allSelectionScopes.has(id);

export const getFocusedSelectionScope = (activeElement: Element | null): SelectionScope | null => {
	// closest() so focus on a control inside a pane (a row button, the diff
	// scroller) still reports that pane's scope.
	const selectionScope = activeElement
		?.closest("[data-selection-scope]")
		?.getAttribute("data-selection-scope");
	if (selectionScope == undefined) return null;
	return isSelectionScope(selectionScope) ? selectionScope : null;
};

/** Whether keyboard focus currently sits inside the given selection scope. */
export const isFocusWithinSelectionScope = (scope: SelectionScope): boolean =>
	document.activeElement?.closest(`[data-selection-scope="${scope}"]`) != null;

/**
 * Whether bare-arrow pane navigation may act for the current focus. Widgets
 * with their own arrow keys keep them: anything outside every pane (dialogs,
 * toasts), toolbars (which rove with arrows wherever they live), and the
 * details header chrome — focus that resolves to the "details" scope itself
 * rather than one of its child panes. Body counts as navigable so the keys
 * can enter a pane when nothing holds focus.
 */
const paneNavigationAllowed = (): boolean => {
	const activeElement = document.activeElement;
	if (activeElement === null || activeElement === document.body) return true;
	if (activeElement.closest('[role="toolbar"]') !== null) return false;

	const scope = getFocusedSelectionScope(activeElement);
	return scope !== null && scope !== "details";
};

const findFocusTarget = (parent: ParentNode, scope: SelectionScope): HTMLElement | null => {
	const root = parent.querySelector<HTMLElement>(`[data-selection-scope="${scope}"]`);
	if (!root) return null;

	const children = selectionScopeChildren[scope];
	if (children) {
		for (const childScope of children) {
			const target = findFocusTarget(root, childScope);
			if (target) return target;
		}
	}

	return root;
};

export const focusSelectionScope = (scope: SelectionScope) => {
	findFocusTarget(document, scope)?.focus({ focusVisible: false });
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
	if (!paneNavigationAllowed()) return;

	const currentSelectionScope = getFocusedSelectionScope(document.activeElement);
	const currentOutlineSelectionScope =
		currentSelectionScope === "uncommitted-files" || currentSelectionScope === "outline"
			? currentSelectionScope
			: outlineSelectionScope;

	// "details" resolves to whichever of its child scopes is mounted (diff or
	// pr tab), so the rightmost slot works on both tabs.
	const orderedSelectionScopes: Array<SelectionScope> = [
		...(outlineVisible ? [currentOutlineSelectionScope ?? "outline"] : []),
		...(filesVisible ? (["files"] satisfies Array<SelectionScope>) : []),
		"details",
	];
	const positionScope =
		currentSelectionScope === "diff" || currentSelectionScope === "pr"
			? "details"
			: currentSelectionScope;

	if (positionScope === null || !orderedSelectionScopes.includes(positionScope)) {
		const nextSelectionScope: SelectionScope | undefined =
			offset === 1 ? orderedSelectionScopes.at(0) : orderedSelectionScopes.at(-1);

		if (nextSelectionScope !== undefined) focusSelectionScope(nextSelectionScope);
	} else {
		const nextIndex = orderedSelectionScopes.indexOf(positionScope) + offset;
		const nextSelectionScope = nextIndex < 0 ? undefined : orderedSelectionScopes.at(nextIndex);
		if (nextSelectionScope !== undefined) focusSelectionScope(nextSelectionScope);
	}
};

/**
 * Returns a ref callback that focuses the scope when its element first attaches.
 *
 * Only the first attachment gets a chance to autofocus: `Activity` detaches and re-attaches refs
 * as it hides and reveals a subtree, and focusing on a reveal would switch the details pane away
 * from whatever the user had selected before hiding it.
 */
export const useAutofocusSelectionScope = () => {
	const attached = useRef(false);

	return (el: HTMLElement | null) => {
		if (el === null || attached.current) return;
		attached.current = true;

		// Don't steal focus if this component is mounted later on.
		if (document.activeElement !== document.body) return;

		el.focus({ focusVisible: false });
	};
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
	onEdgeSpill,
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
	/**
	 * Called with the direction when arrow navigation hits the pane's edge (or
	 * the pane is empty). Wired by whichever component stacks this pane against
	 * a neighbor, so crossing the boundary can enter the adjacent pane.
	 */
	onEdgeSpill?: (offset: -1 | 1) => void;
	getKey: (item: T) => string;
}) => {
	const dispatch = useAppDispatch();

	const moveSelection = (offset: -1 | 1) => {
		const newItem =
			selection === null
				? navigationIndex.items.at(offset === 1 ? 0 : -1)
				: getAdjacent({ navigationIndex, selection, offset, getKey });
		if (newItem === null || newItem === undefined) {
			onEdgeSpill?.(offset);
			return;
		}
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
