import { classes } from "#ui/components/classes.ts";
import type { ComponentProps, FC } from "react";
import styles from "./DiffStats.module.css";

type Props = {
	added: number;
	removed: number;
} & ComponentProps<"span">;

/**
 * The `+N -N` line counts of a diff. Renders nothing when nothing changed, and
 * drops either side when it is zero, so a header shows only what it has to say.
 */
export const DiffStats: FC<Props> = ({ added, removed, ...props }) => {
	if (added === 0 && removed === 0) return null;

	return (
		<span {...props} className={classes(props.className, styles.container)}>
			{added > 0 && <span className={styles.added}>+{added}</span>}
			{removed > 0 && <span className={styles.removed}>-{removed}</span>}
		</span>
	);
};
