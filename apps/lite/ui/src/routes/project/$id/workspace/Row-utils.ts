import { type ButtonSize, type ButtonVariant, getButtonClassName } from "#ui/components/Button.tsx";
import { addressIdentityKey, type Address } from "#ui/addresses.ts";
import { useCursorMatches } from "#ui/use-cursor.ts";

export const treeItemId = (address: Address): string =>
	`sidebar-treeitem-${encodeURIComponent(addressIdentityKey(address))}`;

/**
 * Whether the stored cursor rests on this address. Rows subscribe to this
 * plain boolean instead of consuming the address space, so index rebuilds
 * (fold, filter, data refresh) do not re-render every row. The list keeps the
 * stored cursor aligned with the resolved selection via
 * `useCursorWriteBack`.
 */
export const useIsSelected = (address: Address, name: "applied" | "unapplied"): boolean =>
	useCursorMatches(name, address);

export const getRowButtonClassName = ({
	variant = "ghost",
	size = "small",
	iconOnly = false,
}: {
	variant?: Extract<ButtonVariant, "ghost" | "outline">;
	size?: ButtonSize;
	iconOnly?: boolean;
}) => getButtonClassName({ variant, size, iconOnly });

/** One title line plus a metadata line; shared by both commit list virtualizers. */
export const COMMIT_ROW_HEIGHT = 54;
