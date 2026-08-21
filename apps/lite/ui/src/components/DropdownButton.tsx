import { getButtonClassName, type ButtonSize } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { Button, Tooltip } from "@base-ui/react";
import type { FC, ReactNode } from "react";
import styles from "./DropdownButton.module.css";

/**
 * The Button variants this borrows chrome from.
 *
 * @public
 */
export type DropdownButtonVariant = "pop" | "gray" | "outline";

type Props = {
	/** Contents of the main action button. */
	children: ReactNode;
	/** Accessible name for the chevron trigger, e.g. "Commit options". */
	menuLabel: string;
	/**
	 * Called with the chevron trigger element, so the caller can anchor its menu
	 * to it — the menu itself is the caller's, since a native one and a rendered
	 * one attach the same way.
	 */
	onMenuTrigger: (trigger: HTMLButtonElement) => void;
	/**
	 * Shown on hovering the main action — typically why it is unavailable.
	 * Omitting it leaves the tooltip disabled rather than unmounting it, so the
	 * button keeps its tree position (and its focus) as the reason comes and goes.
	 */
	actionTooltip?: ReactNode;
	variant?: DropdownButtonVariant;
	size?: ButtonSize;
	onClick?: () => void;
	/** Disables the main action; the chevron trigger keeps its own flag. */
	disabled?: boolean;
	menuDisabled?: boolean;
	id?: string;
	className?: string;
};

/**
 * A primary action joined to a chevron that opens its menu, reading as one
 * control split by a seam rather than two buttons side by side.
 *
 * Both halves stay focusable while disabled so keyboard users can reach them
 * and find out why they're unavailable — which also keeps a disabled action
 * hoverable, so `actionTooltip` needs no wrapper element to carry it.
 */
export const DropdownButton: FC<Props> = ({
	children,
	menuLabel,
	onMenuTrigger,
	actionTooltip,
	variant = "outline",
	size = "regular",
	onClick,
	disabled = false,
	menuDisabled = false,
	id,
	className,
}) => (
	<div className={classes(styles.dropdownButton, styles[variant], className)}>
		<Tooltip.Root disabled={actionTooltip === undefined}>
			<Tooltip.Trigger
				id={id}
				className={classes(getButtonClassName({ variant, size }), styles.action)}
				onClick={onClick}
				render={<Button focusableWhenDisabled disabled={disabled} />}
			>
				{children}
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>{actionTooltip}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
		<div aria-hidden className={styles.separator} />
		<Button
			aria-label={menuLabel}
			className={classes(getButtonClassName({ variant, size, iconOnly: true }), styles.trigger)}
			onClick={(event) => onMenuTrigger(event.currentTarget)}
			focusableWhenDisabled
			disabled={menuDisabled}
		>
			<Icon name="chevron-down" />
		</Button>
	</div>
);
