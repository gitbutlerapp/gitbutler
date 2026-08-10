import { Icon } from "#ui/components/Icon.tsx";
import { classes } from "#ui/components/classes.ts";
import type { ComponentProps, FC, ReactNode } from "react";
import styles from "./FilesTree.module.css";

/**
 * The indent rail on the left of a tree row: one step per directory the row
 * sits inside, each drawing the line that ties the row back to that directory,
 * plus — on a directory row — the fold toggle in the step it owns.
 *
 * Nothing is rendered in list mode, where every row is at depth zero and has no
 * toggle of its own.
 */
export const TreeSteps: FC<{ depth: number; children?: ReactNode }> = ({ depth, children }) => {
	if (depth === 0 && children === undefined) return null;

	return (
		<div className={styles.steps}>
			{Array.from({ length: depth }, (_, level) => (
				<span key={level} className={classes(styles.step, styles.stepLine)} aria-hidden />
			))}
			{children}
		</div>
	);
};

/**
 * The chevron that folds a directory. It takes the click on its own so the rest
 * of the row is left to select, the way it is for a file.
 */
export const TreeStepsToggle: FC<{ isCollapsed: boolean } & ComponentProps<"button">> = ({
	isCollapsed,
	...props
}) => (
	<button
		type="button"
		// The tree moves with the arrow keys rather than Tab, and the same fold is
		// on the z hotkey, so this stays out of the tab order.
		tabIndex={-1}
		{...props}
		aria-expanded={!isCollapsed}
		className={classes(props.className, styles.step, styles.stepToggle)}
	>
		<Icon size={12} name={isCollapsed ? "chevron-right" : "chevron-down"} />
	</button>
);
