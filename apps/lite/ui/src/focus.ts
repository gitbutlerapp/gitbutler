import { useSyncExternalStore } from "react";

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

// Copied from @tanstack/hotkeys@0.8.0:
// https://github.com/TanStack/hotkeys/blob/c73a3a167c979d500e1008341ecad096a6c4e635/packages/hotkeys/src/manager.utils.ts#L48-L77
// oxlint-disable eslint(curly) -- Keep copied implementation identical to upstream.
export function isInputElement(element: EventTarget | null): boolean {
	if (!element) {
		return false;
	}

	if (element instanceof HTMLInputElement) {
		const type = element.type.toLowerCase();
		if (type === "button" || type === "submit" || type === "reset") {
			return false;
		}
		return true;
	}

	if (element instanceof HTMLTextAreaElement || element instanceof HTMLSelectElement) {
		return true;
	}

	// Check for contenteditable elements (includes "true", "", "plaintext-only",
	// and inherited contenteditable from ancestor elements)
	if (element instanceof HTMLElement && element.isContentEditable) {
		return true;
	}

	return false;
}
// oxlint-enable eslint(curly)
