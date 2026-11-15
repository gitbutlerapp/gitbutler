import { Match } from "effect";
import type { ComponentProps, FC } from "react";
import { classes } from "./classes.ts";
import styles from "./StatusLed.module.css";

export type StatusLedVariant =
	/** Lit green: up and fine, as a machine that is online. */
	| "safe"
	/** Lit amber: up but needs a look, as a machine that is reconnecting. */
	| "warning"
	/** Unlit: not there, as a machine that is offline. */
	| "gray";

/**
 * A small light for whether something is up, as a machine's online light. Say more, such as when
 * it was last seen, in a `Tooltip` around it.
 *
 * Give it an `aria-label` that says what it shows ("Online", "Offline"). Where the words beside it
 * already say so ("live", "all online"), pass `aria-hidden` instead so they aren't read twice.
 *
 * @import import { StatusLed } from "@gitbutler/but-ui-react/StatusLed.tsx";
 */
export const StatusLed: FC<
	{ variant: StatusLedVariant } & ComponentProps<"span"> &
		({ "aria-label": string } | { "aria-hidden": true })
> = ({ variant, ...props }) => (
	<span
		// oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A drawn dot with a name, not a picture with a source.
		role="img"
		{...props}
		className={classes(
			props.className,
			styles.led,
			Match.value(variant).pipe(
				Match.when("safe", () => styles.safe),
				Match.when("warning", () => styles.warning),
				Match.when("gray", () => styles.gray),
				Match.exhaustive,
			),
		)}
	/>
);
