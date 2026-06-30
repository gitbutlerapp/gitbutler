import styles from "./Button.module.css";
import { classes } from "#ui/components/classes.ts";
import { Match } from "effect";

/** @public */
export type ButtonVariant =
	| "pop"
	| "gray"
	| "outline"
	| "outline-inverted"
	| "danger"
	| "ghost"
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
