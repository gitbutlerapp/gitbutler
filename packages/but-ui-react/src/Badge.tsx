import { classes } from "./classes.ts";
import { Match } from "effect";
import { Children, type ComponentProps, type FC, type ReactNode } from "react";
import styles from "./Badge.module.css";

export type BadgeVariant =
	/** The default, and what most badges should be: a count or a neutral label with no verdict. */
	| "fillGray"
	/** `fillGray`'s quieter twin, for the same counts and labels where the solid one is too loud. */
	| "lightGray"
	/** Something that came off or is fine, chosen by meaning rather than weight. */
	| "safe"
	/** Something waiting on the user or at risk, chosen by meaning rather than weight. */
	| "warn"
	/** Something that failed, chosen by meaning rather than weight. */
	| "danger"
	/** Work that has landed, and what an agent wrote: merged and integrated are purple. */
	| "purple"
	/** Something calm and informational, a note rather than a verdict. Not for merged. */
	| "blue"
	/** The accent, and the rarest colour in the app: at most one per surface. */
	| "pop";

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

/**
 * A small pill carrying a count, a status or a short label.
 *
 * An icon leads the label or stands alone; it never trails, and a badge is
 * never a control. Something that opens a menu or acts is a `Button` beside
 * the badge.
 *
 * The label carries the colour over a tint of it, so the pill stays near white.
 *
 * @figma The drawing shows the light theme's tint in both themes: on a dark ground the code tints
 * from the variant's fill instead, and Figma can't change a layer's opacity by mode.
 * @see packages/but-ui/design/components/badges.md
 * @import import { Badge } from "@gitbutler/but-ui-react/Badge.tsx";
 */
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
				Match.exhaustive,
			),
		)}
	>
		{trimTextChildren(children)}
	</span>
);
