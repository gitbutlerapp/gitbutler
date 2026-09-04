import type { Address } from "#ui/addresses.ts";
import type { TransferKind } from "#ui/operations/operation.ts";

export type DragData = {
	sources: Array<Address>;
	/** What dropping performs — a drag from the remote leg copies, the rest move. */
	kind: TransferKind;
};

export const parseDragData = (data: unknown): DragData | null => {
	// Both fields, so a payload from elsewhere cannot reach the operation
	// machinery half-formed and crash it mid-drag.
	if (typeof data !== "object" || data === null || !("sources" in data) || !("kind" in data))
		return null;
	return data as DragData;
};

/**
 * Marks a subtree that should never start an operation drag, so pointer gestures inside it keep
 * their native behaviour (text selection in the commit message, for instance).
 */
export const NO_DRAG_ATTRIBUTE = "data-gitbutler-no-drag";
