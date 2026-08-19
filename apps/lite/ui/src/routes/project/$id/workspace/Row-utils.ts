import { type ButtonSize, type ButtonVariant, getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { operandIdentityKey, type Operand } from "#ui/operands.ts";
import { useCursorMatches } from "#ui/use-cursor.ts";
import { Match } from "effect";
import styles from "./Row.module.css";

export const treeItemId = (operand: Operand): string =>
	`sidebar-treeitem-${encodeURIComponent(operandIdentityKey(operand))}`;

/**
 * Whether the stored cursor rests on this operand. Rows subscribe to this
 * plain boolean instead of consuming the navigation index, so index rebuilds
 * (fold, filter, data refresh) do not re-render every row. The list keeps the
 * stored cursor aligned with the resolved selection via
 * `useCursorWriteBack`.
 */
export const useIsSelected = (
	operand: Operand,
	name: "applied" | "unapplied" | "upstream",
): boolean => useCursorMatches(name, operand);

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
