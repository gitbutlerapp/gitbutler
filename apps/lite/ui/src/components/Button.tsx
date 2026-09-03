import styles from "./Button.module.css";
import { classes } from "#ui/components/classes.ts";
import { Match } from "effect";

/**
 * Ghost and outline are the two quiet buttons and rank equally; gray and pop are the two ways to
 * raise one above the rest. Reach for a quiet one unless there is a reason not to. Gray is how you
 * highlight, pop is how you point — see "Emphasis" in `apps/lite/DESIGN.md`.
 *
 * @public
 */
export type ButtonVariant =
	/** The primary action of the whole surface. At most one per surface: if two things pop, neither does. */
	| "pop"
	/** Solid gray ground. Lifts a button above the ones around it without spending color — a highlight, not a primary action. */
	| "gray"
	/**
	 * A ghost with an edge, for a button on open ground where nothing else marks it as a target.
	 * Ranks equally with `ghost`: mixing the two in one group separates kinds of control (the PR
	 * toolbar outlines its Auto-merge toggle among ghost actions), never levels of importance.
	 */
	| "outline"
	/** `outline` for a button on an inverted ground — a selected row, not dark mode. */
	| "outline-inverted"
	/** For an act the user cannot take back: deleting, discarding, hard-resetting. Chosen by consequence, so it sits outside the ladder. */
	| "danger"
	/** No ground, no border. The default, for actions inside something already a container: a row, a toolbar, a popup. */
	| "ghost"
	/** `ghost` for a button on an inverted ground — a selected row, not dark mode. */
	| "ghost-inverted";
/** @public */
export type ButtonSize = "regular" | "small";

/** @public */
export type ButtonStyleProps = {
	variant?: ButtonVariant;
	size?: ButtonSize;
	iconOnly?: boolean;
	disableTransition?: boolean;
};

export const getButtonClassName = ({
	variant = "outline",
	size = "regular",
	iconOnly = false,
	disableTransition = false,
}: ButtonStyleProps) =>
	classes(
		"text-semibold",
		styles.button,
		styles[variant],
		Match.value(size).pipe(
			Match.when("small", () => classes(styles.small, "text-12")),
			Match.when("regular", () => classes(styles.regular, "text-13")),
			Match.exhaustive,
		),
		iconOnly && styles.iconOnly,
		disableTransition && styles.disableTransition,
	);
