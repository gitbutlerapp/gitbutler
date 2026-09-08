import { classes } from "#ui/components/classes.ts";
import type { FC } from "react";
import { Separator, type SeparatorProps } from "react-resizable-panels";
import styles from "./ResizeHandle.module.css";

/**
 * The hairline divider between resizable panels. It picks its own axis from
 * the separator's `aria-orientation`, so the same handle works in both
 * horizontal and vertical `Group`s. `grab="after"` puts the whole grab area
 * past the hairline, for a previous panel whose scrollbar meets it.
 */
export const ResizeHandle: FC<SeparatorProps & { grab?: "after" }> = ({ grab, ...p }) => (
	<Separator
		{...p}
		className={classes(styles.resizeHandle, grab === "after" && styles.grabAfter, p.className)}
	/>
);
