import { classes } from "#ui/components/classes.ts";
import { Match } from "effect";
import { Children, type ComponentProps, type FC, type ReactNode } from "react";
import styles from "./Badge.module.css";

export type BadgeVariant =
	| "fillGray"
	| "lightGray"
	| "safe"
	| "warn"
	| "danger"
	| "purple"
	| "blue"
	| "pop"
	| "integrated";

/** @public */
export type BadgeSize = "regular" | "large";

/**
 * `text-box-trim` only applies to the box that directly contains the text, and it
 * is not inherited, so a bare text child of the flex container lands in an
 * anonymous item we cannot target. Wrapping just the text lets the badge center
 * it on its cap-height box rather than its line box; icons stay flex siblings so
 * the gap between them and the label still applies.
 */
const trimTextChildren = (children: ReactNode) =>
	Children.map(children, (child) =>
		typeof child === "string" || typeof child === "number" ? (
			<span className={styles.label}>{child}</span>
		) : (
			child
		),
	);

export const Badge: FC<{ variant: BadgeVariant; size?: BadgeSize } & ComponentProps<"span">> = ({
	variant,
	size = "regular",
	children,
	...props
}) => (
	<span
		{...props}
		className={classes(
			props.className,
			"text-semibold",
			styles.badge,
			Match.value(size).pipe(
				Match.when("regular", () => classes(styles.regular, "text-11")),
				Match.when("large", () => classes(styles.large, "text-12")),
				Match.exhaustive,
			),
			Match.value(variant).pipe(
				Match.when("fillGray", () => styles.fillGray),
				Match.when("lightGray", () => styles.lightGray),
				Match.when("safe", () => styles.safe),
				Match.when("warn", () => styles.warn),
				Match.when("danger", () => styles.danger),
				Match.when("purple", () => styles.purple),
				Match.when("blue", () => styles.blue),
				Match.when("pop", () => styles.pop),
				Match.when("integrated", () => styles.integrated),
				Match.exhaustive,
			),
		)}
	>
		{trimTextChildren(children)}
	</span>
);
