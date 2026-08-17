import { classes } from "#ui/components/classes.ts";
import type { ComponentProps, FC } from "react";
import styles from "./StackCard.module.css";

/**
 * The card a stack is drawn in, shared by the workspace outline and the
 * branches tab: a full-width container holding the stack's branches, divided
 * from the next card by its own floor.
 *
 * The card itself is the ARIA group the tree items live in: callers pass its
 * role and label as props and render only the items as children. The body is a
 * plain layout wrapper, so that the group is not nested inside itself.
 */
export const StackCard: FC<ComponentProps<"div">> = ({ children, ...props }) => (
	<div {...props} className={classes(props.className, styles.card)}>
		<div className={styles.body}>{children}</div>
	</div>
);
