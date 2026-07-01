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
