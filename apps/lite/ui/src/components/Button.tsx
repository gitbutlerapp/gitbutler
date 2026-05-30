import styles from "./Button.module.css";
import { classes } from "#ui/components/classes.ts";

export type ButtonVariant = "pop" | "gray" | "outline" | "danger" | "ghost" | "inverted";
export type ButtonSize = "regular" | "small";

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
		size === "small" && styles.small,
		size === "small" ? "text-12" : "text-13",
		iconOnly && styles.iconOnly,
	);
