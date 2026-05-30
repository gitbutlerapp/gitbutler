import styles from "./Button.module.css";
import { classes } from "#ui/components/classes.ts";
import { ComponentProps, ReactNode, forwardRef, isValidElement } from "react";

type ButtonVariant = "pop" | "gray" | "outline" | "danger" | "ghost" | "inverted";
type ButtonSize = "regular" | "small";

type Props = ComponentProps<"button"> & {
	variant?: ButtonVariant;
	size?: ButtonSize;
	children?: ReactNode;
};

function hasLabelContent(children: ReactNode): boolean {
	switch (typeof children) {
		case "string":
			return children.trim().length > 0;
		case "number":
			return true;
	}

	if (children === null || children === undefined || children === false) return false;

	if (Array.isArray(children))
		return (children as Array<ReactNode>).some((child) => hasLabelContent(child));

	if (isValidElement<{ children?: ReactNode }>(children))
		return hasLabelContent(children.props.children);

	return false;
}

export const getButtonClassName = ({
	variant = "outline",
	size = "regular",
	iconOnly,
}: {
	variant?: ButtonVariant;
	size?: ButtonSize;
	iconOnly: boolean;
}) =>
	classes(
		"text-semibold",
		styles.button,
		styles[variant],
		size === "small" && styles.small,
		size === "small" ? "text-12" : "text-13",
		iconOnly && styles.iconOnly,
	);

export const Button = forwardRef<HTMLButtonElement, Props>(function Button(
	{ variant = "outline", size = "regular", children, type = "button", className, ...props },
	ref,
) {
	const iconOnly = !hasLabelContent(children);

	return (
		<button
			{...props}
			ref={ref}
			// oxlint-disable-next-line react/button-has-type -- False positive.
			type={type}
			className={classes(getButtonClassName({ variant, size, iconOnly }), className)}
		>
			{children}
		</button>
	);
});
