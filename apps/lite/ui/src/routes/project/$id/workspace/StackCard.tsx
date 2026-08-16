import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import type { IconName } from "#ui/components/iconNames.ts";
import { Toolbar } from "@base-ui/react";
import type { ComponentProps, FC, ReactNode } from "react";
import { Row, RowToolbar } from "./Row.tsx";
import styles from "./StackCard.module.css";

/**
 * The card a stack is drawn in, shared by the workspace outline and the
 * branches tab: a full-width container holding an optional header above a
 * body.
 *
 * The card itself is the ARIA group the tree items live in: callers pass its
 * role and label as props and render only the items as children. The body is a
 * plain layout wrapper, so that the group is not nested inside itself.
 */
export const StackCard: FC<{ header?: ReactNode } & ComponentProps<"div">> = ({
	header,
	children,
	...props
}) => (
	<div {...props} className={classes(props.className, styles.card)}>
		{header}

		<div className={styles.body}>{children}</div>
	</div>
);

/**
 * A {@link StackCard} header: a non-interactive strip of chrome naming what the
 * card holds, divided from the body below it. `children` are the toolbar's
 * buttons, which sit at the header's trailing edge.
 */
export const StackCardHeader: FC<
	{
		/** Glyph for what the card holds, shown before `label`. */
		icon: IconName;
		/** Names what the card holds, e.g. "Branches in workspace". */
		label: ReactNode;
		/** Accessible name for the header's toolbar. */
		toolbarLabel: string;
	} & Omit<ComponentProps<typeof Row>, "interactive" | "onSelect" | "isSelected">
> = ({ icon, label, toolbarLabel, children, ...props }) => (
	<Row {...props} interactive={false} className={classes(props.className, styles.header)}>
		<span className={styles.headerContent}>
			<Icon name={icon} size={14} className={styles.headerIcon} />
			<span className={classes("text-12", styles.headerLabel)}>{label}</span>
		</span>

		<Toolbar.Root
			aria-label={toolbarLabel}
			render={<RowToolbar forceVisible className={styles.headerToolbar} />}
		>
			{children}
		</Toolbar.Root>
	</Row>
);
