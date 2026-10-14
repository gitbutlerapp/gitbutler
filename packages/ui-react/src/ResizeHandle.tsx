import { classes } from "./classes.ts";
import type { FC } from "react";
import { Separator, type SeparatorProps } from "react-resizable-panels";
import styles from "./ResizeHandle.module.css";

/**
 * The hairline divider between resizable panels. It picks its own axis from
 * the separator's `aria-orientation`, so the same handle works in both
 * horizontal and vertical `Group`s. `grab="after"` puts the whole grab area
 * past the hairline, for a previous panel whose scrollbar meets it. `gap`
 * draws no hairline: the handle is the space between panels set apart as
 * cards, and shows a short grip on hover that darkens and grows while
 * dragged.
 *
 * @import import { ResizeHandle } from "@gitbutler/ui-react/ResizeHandle.tsx";
 */
export const ResizeHandle: FC<SeparatorProps & { grab?: "after"; gap?: boolean }> = ({
	grab,
	gap,
	...p
}) => (
	<Separator
		{...p}
		className={classes(
			styles.resizeHandle,
			grab === "after" && styles.grabAfter,
			gap && styles.gap,
			p.className,
		)}
	/>
);
