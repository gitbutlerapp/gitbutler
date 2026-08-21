import { useSyncExternalStore } from "react";
import type { FocusScope } from "#ui/focus-scopes.ts";

const allFocusScopes: Record<FocusScope, null> = {
	details: null,
	"uncommitted-files": null,
	sidebar: null,
	files: null,
	diff: null,
	pr: null,
};

/**
 * Kept in this lower-level module so `use-cursor.ts` can validate a DOM scope without importing
 * runtime code from `focus-scopes.ts`, which already imports `use-cursor.ts`. The `FocusScope`
 * type remains there because its type-only import is erased and does not create that cycle.
 */
export const isFocusScope = (id: string): id is FocusScope => Object.hasOwn(allFocusScopes, id);

const subscribeToFocus = (onStoreChange: () => void) => {
	window.addEventListener("focusin", onStoreChange);
	window.addEventListener("focusout", onStoreChange);

	return () => {
		window.removeEventListener("focusin", onStoreChange);
		window.removeEventListener("focusout", onStoreChange);
	};
};

export const useActiveElement = () =>
	useSyncExternalStore(
		subscribeToFocus,
		() => document.activeElement,
		() => null,
	);
