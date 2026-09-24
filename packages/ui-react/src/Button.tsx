import styles from "./Button.module.css";
import { classes } from "./classes.ts";
import { Button as BaseButton } from "@base-ui/react/button";
import { Match } from "effect";
import type { FC, Ref } from "react";

/**
 * Ghost and outline are the two quiet buttons and rank equally; gray and pop are the two ways to
 * raise one above the rest. Reach for a quiet one unless there is a reason not to. Gray is how you
 * highlight, pop is how you point — see "Emphasis" in `packages/ui-react/DESIGN.md`.
 *
 * @public
 */
export type ButtonVariant =
	/** The primary action of the whole surface. At most one per surface: if two things pop, neither does. */
	| "pop"
	/** Solid gray ground. Lifts a button above the ones around it without spending color — a highlight, not a primary action. */
	| "gray"
	/**
	 * The default. A ghost with an edge, for a button on open ground where nothing else marks it as a target.
	 * Ranks equally with `ghost`: mixing the two in one group separates kinds of control (the PR
	 * toolbar outlines its Auto-merge toggle among ghost actions), never levels of importance.
	 */
	| "outline"
	/** `outline` for a button on an inverted ground — a selected row, not dark mode. */
	| "outline-inverted"
	/** For an act the user cannot take back: deleting, discarding, hard-resetting. Chosen by consequence, so it sits outside the ladder. */
	| "danger"
	/** No ground, no border. For actions inside something already a container: a row, a toolbar, a popup. */
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

// oxlint-disable-next-line react/only-export-components -- The button and the styles other components borrow to look like one belong together.
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

/** @public */
export type ButtonProps = BaseButton.Props & ButtonStyleProps & { ref?: Ref<HTMLElement> };

/**
 * The button: Base UI's, so `focusableWhenDisabled` and `render` work, styled by variant and size.
 * An icon goes in as a child, before or after the label; an icon-only button takes `iconOnly` and
 * needs an `aria-label`. It is `type="button"` unless told otherwise, so one inside a form doesn't
 * submit it by accident.
 *
 * To make another component look like a button — a toolbar item, a number field's stepper — give it
 * `getButtonClassName` instead; to make a Base UI trigger one, pass this as its `render`.
 *
 * @import import { Button } from "@gitbutler/ui-react/Button.tsx";
 */
export const Button: FC<ButtonProps> = ({
	variant,
	size,
	iconOnly,
	disableTransition,
	className,
	type = "button",
	...props
}) => {
	const own = getButtonClassName({ variant, size, iconOnly, disableTransition });
	return (
		<BaseButton
			{...props}
			type={type}
			className={
				typeof className === "function"
					? (state) => classes(own, className(state))
					: classes(own, className)
			}
		/>
	);
};
