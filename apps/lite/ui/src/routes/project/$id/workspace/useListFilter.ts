import { changesFileHotkeys } from "#ui/hotkeys.ts";
import { focusScope, type FocusScope } from "#ui/focus-scopes.ts";
import { useHotkey } from "@tanstack/react-hotkeys";
import type { ComponentProps, RefObject } from "react";
import type { ListFilterRow } from "./ListFilterRow.tsx";

/**
 * Focused by id rather than by ref because the input is only mounted while the
 * filter is open, so a ref would still be null where the hotkeys are bound.
 */
const focusFilterInput = (inputId: string) => {
	const input = document.getElementById(inputId);
	if (!(input instanceof HTMLInputElement)) return;

	input.focus();
	// Land the caret after the query, so returning to it extends the search
	// rather than overwriting it.
	input.setSelectionRange(input.value.length, input.value.length);
};

/**
 * The keyboard loop around a list's filter: a hotkey in, down into the list, up
 * at its top back to the query, escape out. The query itself lives with
 * whichever list is being filtered — this only needs to read it and hand back a
 * new one.
 *
 * Rows are named by key rather than by value so lists of any item type can use
 * it: only the comparison "is the selection the first row" is needed here, and
 * `onEnterList` closes over whatever the caller has to select.
 *
 * Pass `panelRef` for the region the hotkey should reach (header, list, and
 * anything else that belongs to the same list) and `listRef` for the list
 * itself, then render {@link ListFilterRow} with `rowProps` whenever it is not
 * null.
 */
export const useListFilter = ({
	filter,
	setFilter,
	inputId,
	subject,
	scope,
	selectionKey,
	firstKey,
	onEnterList,
	panelRef,
	listRef,
	enabled = true,
}: {
	/** The query, or `null` while the filter is closed. */
	filter: string | null;
	setFilter: (filter: string | null) => void;
	/** Unique per list: two filters can be open at once. */
	inputId: string;
	/** What is being filtered, plural and lowercase — "files", "branches". */
	subject: string;
	scope: FocusScope;
	/** Identifies the list's resolved selection; `null` when it has none. */
	selectionKey: string | null;
	/** Identifies the first row of the filtered list, where up leaves for the input again. */
	firstKey: string | undefined;
	/** Previews the list's resolved selection, then hands it focus. */
	onEnterList: () => void;
	/** The region the open hotkey reaches: the list and whatever else belongs to it. */
	panelRef: RefObject<HTMLElement | null>;
	listRef: RefObject<HTMLElement | null>;
	/** Set false where the list is hidden, so the hotkey does not open a filter nobody can see. */
	enabled?: boolean;
}): {
	open: () => void;
	rowProps: ComponentProps<typeof ListFilterRow> | null;
} => {
	const open = () => {
		// The input takes focus itself when it mounts.
		if (filter === null) setFilter("");
		else focusFilterInput(inputId);
	};

	const close = () => {
		setFilter(null);
		// Hand focus back to the list the filter was narrowing, rather than
		// dropping it on the body when the input unmounts.
		focusScope(scope);
	};

	const enterList = () => {
		// A list resolves its selection against the filtered rows, so this previews
		// a match even when the filter narrowed away whatever was selected before.
		// Selecting it stores that resolution, keeping the row highlight and the
		// details pane in step.
		onEnterList();
		focusScope(scope);
	};

	useHotkey(changesFileHotkeys.filter.hotkey, open, {
		conflictBehavior: "allow",
		enabled,
		meta: changesFileHotkeys.filter.meta,
		target: panelRef,
	});

	// The way out of the list is the way in, reversed: up from the top row lands
	// back in the query. Only bound while filtering, so it does not swallow the
	// stop at the top of an unfiltered list.
	useHotkey("ArrowUp", () => focusFilterInput(inputId), {
		conflictBehavior: "allow",
		enabled: filter !== null && (selectionKey === null || selectionKey === firstKey),
		target: listRef,
	});

	return {
		open,
		rowProps:
			filter === null
				? null
				: {
						filter,
						inputId,
						subject,
						onFilterChange: setFilter,
						onClose: close,
						onEnterList: enterList,
					},
	};
};
