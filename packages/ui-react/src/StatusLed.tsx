import type { ComponentProps, FC } from "react";
import { classes } from "./classes.ts";
import styles from "./StatusLed.module.css";

/**
 * A machine's online light: lit green while it is connected, grey once it isn't. Say when it was
 * last seen in a `Tooltip` around it.
 *
 * It names itself Online or Offline to a screen reader. Where the words beside it already say so
 * ("live", "all online"), pass `aria-hidden` so they aren't read twice.
 *
 * @import import { StatusLed } from "@gitbutler/ui-react/StatusLed.tsx";
 */
export const StatusLed: FC<{ online: boolean } & ComponentProps<"span">> = ({
	online,
	...props
}) => (
	<span
		// oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A drawn dot with a name, not a picture with a source.
		role="img"
		aria-label={online ? "Online" : "Offline"}
		{...props}
		className={classes(props.className, styles.led, online && styles.online)}
	/>
);
