import styles from "./Button.module.css";
import { classes } from "#ui/components/classes.ts";

/** @public */
export type ButtonVariant = "pop" | "gray" | "outline" | "danger" | "ghost" | "inverted";
/** @public */
export type ButtonSize = "regular" | "small";

/** @public */
export type ButtonStyleProps = {
	variant?: ButtonVariant;
	size?: ButtonSize;
	iconOnly?: boolean;
};

export const getButtonClassName = ({
	variant = "outline",
	size = "regular",
	iconOnly = false,
}: ButtonStyleProps) =>
	classes(
		"text-semibold",
		styles.button,
		styles[variant],
		(() => {
			switch (size) {
				case "small":
					return classes(styles.small, "text-12");
				case "regular":
					return classes(styles.regular, "text-13");
			}
		})(),
		iconOnly && styles.iconOnly,
	);
