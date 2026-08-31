import type { Address } from "#ui/addresses.ts";

export type DragData = {
	sources: Array<Address>;
};

export const parseDragData = (data: unknown): DragData | null => {
	if (typeof data !== "object" || data === null || !("sources" in data)) return null;
	return data as DragData;
};

/**
 * Marks a subtree that should never start an operation drag, so pointer gestures inside it keep
 * their native behaviour (text selection in the commit message, for instance).
 */
export const NO_DRAG_ATTRIBUTE = "data-gitbutler-no-drag";
