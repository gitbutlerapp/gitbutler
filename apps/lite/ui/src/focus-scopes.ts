import { selectionOperationHotkeys, type CommandGroup } from "#ui/hotkeys.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { startKeyboardTransfer } from "#ui/use-cursor.ts";
import type { Placement } from "#ui/operations/operation.ts";
import type { Address } from "#ui/addresses.ts";
import { getAdjacent, type AddressSpace } from "#ui/workspace/address-space.ts";
import { useHotkeySequences, useHotkeys } from "@tanstack/react-hotkeys";
import { useRef } from "react";

export type FocusScope = "details" | "uncommitted-files" | "sidebar" | "files" | "diff" | "pr";
const allFocusScopes: Set<string> = new Set([
	"details",
	"uncommitted-files",
	"sidebar",
	"files",
	"diff",
	"pr",
] satisfies Array<FocusScope>);

// Supports arbitrarily nested scopes. Must share that ancestral relationship in the DOM. Tries from
// left to right.
const focusScopeChildren: Partial<Record<FocusScope, Set<FocusScope>>> = {
	details: new Set(["diff", "pr", "files"]),
};

const isFocusScope = (id: string): id is FocusScope => allFocusScopes.has(id);

export const getFocusedScope = (activeElement: Element | null): FocusScope | null => {
	// closest() so focus on a control inside a pane (a row button, the diff
	// scroller) still reports that pane's scope.
	const focusScope = activeElement?.closest("[data-focus-scope]")?.getAttribute("data-focus-scope");
	if (focusScope == undefined) return null;
	return isFocusScope(focusScope) ? focusScope : null;
};

/** Whether keyboard focus currently sits inside the given selection scope. */
export const isFocusWithinScope = (scope: FocusScope): boolean =>
	document.activeElement?.closest(`[data-focus-scope="${scope}"]`) != null;

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

	const scope = getFocusedScope(activeElement);
	return scope !== null && scope !== "details";
};

const findFocusTarget = (parent: ParentNode, scope: FocusScope): HTMLElement | null => {
	const root = parent.querySelector<HTMLElement>(`[data-focus-scope="${scope}"]`);
	if (!root) return null;

	const children = focusScopeChildren[scope];
	if (children) {
		for (const childScope of children) {
			const target = findFocusTarget(root, childScope);
			if (target) return target;
		}
	}

	return root;
};

export const focusScope = (scope: FocusScope) => {
	findFocusTarget(document, scope)?.focus({ focusVisible: false });
};

export const focusHorizontalScope = ({
	filesVisible,
	offset,
	sidebarFocusScope,
	detailsFullWindow,
}: {
	filesVisible: boolean;
	offset: -1 | 1;
	sidebarFocusScope: Extract<FocusScope, "uncommitted-files" | "sidebar"> | null;
	detailsFullWindow: boolean;
}) => {
	if (!paneNavigationAllowed()) return;

	const currentFocusScope = getFocusedScope(document.activeElement);
	const currentSidebarFocusScope =
		currentFocusScope === "uncommitted-files" || currentFocusScope === "sidebar"
			? currentFocusScope
			: sidebarFocusScope;

	// "details" resolves to whichever of its child scopes is mounted (diff or
	// pr tab), so the rightmost slot works on both tabs.
	const orderedFocusScopes: Array<FocusScope> = [
		...(detailsFullWindow ? [] : [currentSidebarFocusScope ?? "sidebar"]),
		...(filesVisible ? (["files"] satisfies Array<FocusScope>) : []),
		"details",
	];
	const positionScope =
		currentFocusScope === "diff" || currentFocusScope === "pr" ? "details" : currentFocusScope;

	if (positionScope === null || !orderedFocusScopes.includes(positionScope)) {
		const nextFocusScope: FocusScope | undefined =
			offset === 1 ? orderedFocusScopes.at(0) : orderedFocusScopes.at(-1);

		if (nextFocusScope !== undefined) focusScope(nextFocusScope);
	} else {
		const nextIndex = orderedFocusScopes.indexOf(positionScope) + offset;
		const nextFocusScope = nextIndex < 0 ? undefined : orderedFocusScopes.at(nextIndex);
		if (nextFocusScope !== undefined) focusScope(nextFocusScope);
	}
};

/**
 * Returns a ref callback that focuses the scope when its element first attaches.
 *
 * Only the first attachment gets a chance to autofocus: `Activity` detaches and re-attaches refs
 * as it hides and reveals a subtree, and focusing on a reveal would switch the details pane away
 * from whatever the user had selected before hiding it.
 */
export const useAutofocusScope = (enabled = true) => {
	const attached = useRef(false);

	return (el: HTMLElement | null) => {
		if (el === null || attached.current) return;

		// A list that is not the active one must not take focus on
		// mount: its onFocus would then name it the active list, so autofocus
		// would decide where the URL says the user is. Checked before the latch,
		// which records having taken the one autofocus rather than having seen a
		// ref, so a list that starts out passive can still take it later.
		if (!enabled) return;

		attached.current = true;

		// Don't steal focus if this component is mounted later on.
		if (document.activeElement !== document.body) return;

		el.focus({ focusVisible: false });
	};
};

export const useAddressSpaceHotkeys = <T>({
	addressSpace,
	group,
	select,
	selection,
	ref,
	selectSectionPredicate,
	operationSourcesForItem,
	onEdgeSpill,
	getKey,
	directionalNavigation = true,
	projectId,
}: {
	projectId: string;
	addressSpace: AddressSpace<T>;
	group: CommandGroup;
	select: (newItem: T) => void;
	selection: T | null;
	ref: React.RefObject<HTMLElement | null>;
	selectSectionPredicate?: (item: T) => boolean;
	/** When omitted, the selection operation hotkeys (move, cut) are not registered. */
	operationSourcesForItem?: (item: T) => Array<Address>;
	/**
	 * Called with the direction when arrow navigation hits the pane's edge (or
	 * the pane is empty). Wired by whichever component stacks this pane against
	 * a neighbor, so crossing the boundary can enter the adjacent pane.
	 */
	onEdgeSpill?: (offset: -1 | 1) => void;
	getKey: (item: T) => string;
	/** Disable arrow and Vim directional keys when a surface owns finer-grained navigation. */
	directionalNavigation?: boolean;
}) => {
	const operationHotkeysEnabled = useAppSelector(
		(state) =>
			operationSourcesForItem === undefined ||
			projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);

	const moveSelection = (offset: -1 | 1) => {
		const newItem =
			selection === null
				? addressSpace.items.at(offset === 1 ? 0 : -1)
				: getAdjacent({ addressSpace, selection, offset, getKey });
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

		const selectionIndex = addressSpace.indexByKey.get(getKey(selection));
		if (selectionIndex === undefined) return;

		const currentItem = addressSpace.items[selectionIndex];
		const startsOnMatch = currentItem !== undefined && predicate(currentItem);
		let itemIndex = selectionIndex + (offset === -1 && !startsOnMatch ? 0 : offset);

		while (itemIndex >= 0 && itemIndex < addressSpace.items.length) {
			const item = addressSpace.items[itemIndex];
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
		const newItem = addressSpace.items[0];
		if (newItem === undefined) return;
		select(newItem);
	};

	const selectLastItem = () => {
		const newItem = addressSpace.items.at(-1);
		if (newItem === undefined) return;
		select(newItem);
	};

	useHotkeys([
		{
			hotkey: "ArrowUp",
			callback: selectPreviousItem,
			options: {
				conflictBehavior: "allow",
				enabled: directionalNavigation,
				target: ref,
			},
		},
		{
			hotkey: "K",
			callback: selectPreviousItem,
			options: {
				conflictBehavior: "allow",
				enabled: directionalNavigation,
				target: ref,
			},
		},
		{
			hotkey: "ArrowDown",
			callback: selectNextItem,
			options: {
				conflictBehavior: "allow",
				enabled: directionalNavigation,
				target: ref,
			},
		},
		{
			hotkey: "J",
			callback: selectNextItem,
			options: {
				conflictBehavior: "allow",
				enabled: directionalNavigation,
				target: ref,
			},
		},
		{
			hotkey: "Shift+ArrowUp",
			callback: selectPreviousSection,
			options: {
				conflictBehavior: "allow",
				enabled: directionalNavigation,
				target: ref,
			},
		},
		{
			hotkey: "Shift+K",
			callback: selectPreviousSection,
			options: {
				conflictBehavior: "allow",
				enabled: directionalNavigation,
				target: ref,
			},
		},
		{
			hotkey: "Shift+ArrowDown",
			callback: selectNextSection,
			options: {
				conflictBehavior: "allow",
				enabled: directionalNavigation,
				target: ref,
			},
		},
		{
			hotkey: "Shift+J",
			callback: selectNextSection,
			options: {
				conflictBehavior: "allow",
				enabled: directionalNavigation,
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

	const startTransferForSelection = (placement: Placement) => {
		if (selection === null || operationSourcesForItem === undefined) return;

		startKeyboardTransfer({
			sources: operationSourcesForItem(selection),
			kind: "move",
			placement,
		});

		focusScope("sidebar");
	};

	useHotkeys([
		{
			hotkey: selectionOperationHotkeys.move.hotkey,
			callback: () => startTransferForSelection("above"),
			options: {
				conflictBehavior: "allow",
				enabled:
					operationHotkeysEnabled && selection !== null && operationSourcesForItem !== undefined,
				target: ref,
				meta: { group, name: "Move" },
			},
		},
		{
			hotkey: selectionOperationHotkeys.cut.hotkey,
			callback: () => startTransferForSelection("into"),
			options: {
				conflictBehavior: "allow",
				enabled:
					operationHotkeysEnabled && selection !== null && operationSourcesForItem !== undefined,
				target: ref,
				ignoreInputs: true,
				meta: { group, name: "Cut" },
			},
		},
	]);
};
