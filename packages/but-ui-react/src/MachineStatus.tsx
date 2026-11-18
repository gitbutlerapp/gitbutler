import type { ComponentProps, FC } from "react";
import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import styles from "./MachineStatus.module.css";

/**
 * Whether a machine is online, beside its name: a signal on green when it is, a crossed-out
 * signal on grey when it isn't. Say more, such as when it was last seen, in a `Tooltip` around it.
 *
 * It says "Online" or "Offline" to a screen reader. Where the words beside it already say so,
 * pass `aria-hidden` so they aren't read twice. For a light with no machine's name beside it,
 * such as "3 of 4 online", use `StatusLed`.
 *
 * @import import { MachineStatus } from "@gitbutler/but-ui-react/MachineStatus.tsx";
 */
export const MachineStatus: FC<{ online: boolean } & ComponentProps<"span">> = ({
	online,
	...props
}) => (
	<span
		// oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A drawn glyph with a name, not a picture with a source.
		role="img"
		aria-label={online ? "Online" : "Offline"}
		{...props}
		className={classes(props.className, styles.status, online ? styles.online : styles.offline)}
	>
		<Icon name={online ? "signal" : "no-signal"} size={12} />
	</span>
);
