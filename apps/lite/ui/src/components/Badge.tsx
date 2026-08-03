import { classes } from "#ui/components/classes.ts";
import { Match } from "effect";
import type { ComponentProps, FC } from "react";
import styles from "./Badge.module.css";

export type BadgeVariant =
	| "fillGray"
	| "lightGray"
	| "safe"
	| "warn"
	| "danger"
	| "purple"
	| "blue"
	| "integrated";

export const Badge: FC<{ variant: BadgeVariant } & ComponentProps<"span">> = ({
	variant,
	...props
}) => (
	<span
		{...props}
		className={classes(
			props.className,
			"text-11",
			"text-semibold",
			styles.badge,
			Match.value(variant).pipe(
				Match.when("fillGray", () => styles.fillGray),
				Match.when("lightGray", () => styles.lightGray),
				Match.when("safe", () => styles.safe),
				Match.when("warn", () => styles.warn),
				Match.when("danger", () => styles.danger),
				Match.when("purple", () => styles.purple),
				Match.when("blue", () => styles.blue),
				Match.when("integrated", () => styles.integrated),
				Match.exhaustive,
			),
		)}
	/>
);
