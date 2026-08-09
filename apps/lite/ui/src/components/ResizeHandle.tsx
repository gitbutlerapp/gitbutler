import { classes } from "#ui/components/classes.ts";
import type { FC } from "react";
import { Separator, type SeparatorProps } from "react-resizable-panels";
import styles from "./ResizeHandle.module.css";

/**
 * The hairline divider between resizable panels. It picks its own axis from
 * the separator's `aria-orientation`, so the same handle works in both
 * horizontal and vertical `Group`s.
 */
export const ResizeHandle: FC<SeparatorProps> = (p) => (
	<Separator {...p} className={classes(styles.resizeHandle, p.className)} />
);
