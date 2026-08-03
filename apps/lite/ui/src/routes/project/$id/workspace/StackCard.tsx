import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { Toolbar, Tooltip } from "@base-ui/react";
import type { ComponentProps, FC, ReactNode } from "react";
import { Row, RowToolbar } from "./Row.tsx";
import { getRowButtonClassName } from "./Row-utils.ts";
import styles from "./StackCard.module.css";

/**
 * The card a stack is drawn in, shared by the workspace outline and the
 * branches tab: a rounded, shadowed container holding an optional full-bleed
 * header above an inset body.
 *
 * The card itself is the ARIA group the tree items live in: callers pass its
 * role and label as props and render only the items as children. The body is a
 * plain layout wrapper, so that the group is not nested inside itself.
 */
export const StackCard: FC<
	{
		header?: ReactNode;
		/**
		 * Applied to the inset body wrapping `children`, in the same spirit as
		 * `Scroller`'s `viewportClassName`.
		 */
		bodyClassName?: string;
	} & ComponentProps<"div">
> = ({ header, bodyClassName, children, ...props }) => (
	<div {...props} className={classes(props.className, styles.card)}>
		{header}

		<div className={classes(bodyClassName, styles.body)}>{children}</div>
	</div>
);

/**
 * A {@link StackCard} header: a non-interactive row of chrome, divided from the
 * body below it. `children` are the toolbar's buttons.
 */
export const StackCardHeader: FC<
	{
		/** Accessible name for the header's toolbar. */
		toolbarLabel: string;
	} & Omit<ComponentProps<typeof Row>, "interactive" | "onSelect" | "isSelected">
> = ({ toolbarLabel, children, ...props }) => (
	<Row {...props} interactive={false} className={classes(props.className, styles.header)}>
		<Toolbar.Root
			aria-label={toolbarLabel}
			render={<RowToolbar forceVisible className={styles.headerToolbar} />}
		>
			{children}
		</Toolbar.Root>
	</Row>
);

/**
 * The fold-all control in a {@link StackCardHeader}. A stack holding a single
 * branch has nothing to fold *across* — that branch folds from its own row — so
 * it shows a plain branch glyph in the control's place, keeping the header's
 * slots aligned between the two cases.
 */
export const StackFoldAllButton: FC<{
	hasMultipleBranches: boolean;
	/** Whether any branch in the stack is currently folded. */
	folded: boolean;
	/** @default false */
	disabled?: boolean;
	onToggle: () => void;
}> = ({ hasMultipleBranches, folded, disabled = false, onToggle }) => {
	if (!hasMultipleBranches) {
		return (
			<span
				aria-hidden
				className={classes(getRowButtonClassName({ iconOnly: true }), styles.headerGlyph)}
			>
				<Icon name="branch" />
			</span>
		);
	}

	const label = folded ? "Unfold stack branches" : "Collapse stack branches";

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				aria-label={label}
				className={getRowButtonClassName({ iconOnly: true })}
				render={<Toolbar.Button focusableWhenDisabled disabled={disabled} onClick={onToggle} />}
			>
				<Icon name={folded ? "expand-vertical" : "collapse-vertical"} />
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>{label}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
