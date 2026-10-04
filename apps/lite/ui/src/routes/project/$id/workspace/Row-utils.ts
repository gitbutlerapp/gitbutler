import {
	type ButtonSize,
	type ButtonVariant,
	getButtonClassName,
} from "@gitbutler/ui-react/Button.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { addressIdentityKey, type Address } from "#ui/addresses.ts";
import { useCursorMatches } from "#ui/use-cursor.ts";
import { Match } from "effect";
import type { ComponentProps, MouseEvent } from "react";
import styles from "./Row.module.css";

export const treeItemId = (address: Address): string =>
	`sidebar-treeitem-${encodeURIComponent(addressIdentityKey(address))}`;

/**
 * Whether the stored cursor rests on this address. Rows subscribe to this
 * plain boolean instead of consuming the address space, so index rebuilds
 * (fold, filter, data refresh) do not re-render every row.
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
}) =>
	classes(
		getButtonClassName({
			variant,
			size,
			iconOnly,
			// On selection/focus change we change the button variant. This
			// transition would clash with other selection/focus style changes
			// which are instant (e.g. the row background).
			disableTransition: true,
		}),
		Match.value(variant).pipe(
			Match.when("ghost", () => styles.buttonGhost),
			Match.when("outline", () => styles.buttonOutline),
			Match.exhaustive,
		),
	);

/** A single title line; shared by both commit list virtualizers. */
export const COMMIT_ROW_HEIGHT = 28;

const isFromInteractiveDescendant = (event: MouseEvent<HTMLDivElement>): boolean => {
	if (!(event.target instanceof Element)) return false;
	const interactiveElement = event.target.closest(["button", "input[type='checkbox']"].join(","));
	return interactiveElement !== null && event.currentTarget.contains(interactiveElement);
};

const isFromNonRowBody = (event: MouseEvent<HTMLDivElement>): boolean => {
	if (!(event.target instanceof Element)) return false;
	const interactiveElement = event.target.closest(
		"a, button, input, select, textarea, [contenteditable]",
	);
	return interactiveElement !== null && event.currentTarget.contains(interactiveElement);
};

/**
 * A row's pointer handling, for {@link Row} and for rows drawn by a library component: a click
 * selects it and a shift-click extends, while a click on a control inside it leaves focus with
 * the list.
 */
export const rowPointerProps = ({
	onSelect,
	onShiftSelect,
	onMouseDown,
	onClick,
	onDoubleClick,
}: {
	onSelect?: () => void;
	onShiftSelect?: () => void;
} & Pick<ComponentProps<"div">, "onMouseDown" | "onClick" | "onDoubleClick">): Pick<
	ComponentProps<"div">,
	"onMouseDown" | "onClick" | "onDoubleClick"
> => ({
	onMouseDown: (event) => {
		onMouseDown?.(event);

		if (
			!event.defaultPrevented &&
			// Prevent clicks on interactive descendants from stealing focus from the tree.
			isFromInteractiveDescendant(event)
		)
			event.preventDefault();
	},
	onClick: (event) => {
		onClick?.(event);

		if (event.defaultPrevented || isFromInteractiveDescendant(event)) return;

		if (event.shiftKey && onShiftSelect && !isFromNonRowBody(event)) onShiftSelect();
		else onSelect?.();
	},
	onDoubleClick: (event) => {
		if (!isFromNonRowBody(event)) onDoubleClick?.(event);
	},
});
