import { classes } from "./classes.ts";
import styles from "./Tooltip.module.css";
import { Kbd } from "./Kbd.tsx";
import { Tooltip as BaseTooltip } from "@base-ui/react";
import type { HotkeySequence } from "@tanstack/react-hotkeys";
import { useState, type ComponentProps, type FC, type ReactElement, type ReactNode } from "react";

/**
 * The tooltip's look, for a tooltip `Tooltip` can't build: one whose trigger is detached through a
 * `handle`, or whose content moves between triggers. Give it as `Tooltip.Popup`'s `render`.
 *
 * @import import { TooltipPopup } from "@gitbutler/ui-react/Tooltip.tsx";
 */
export const TooltipPopup: FC<
	ComponentProps<"div"> & {
		/** Optional keyboard shortcut displayed alongside the content. */
		kbd?: string | HotkeySequence;
		/**
		 * The scope the shortcut is bound to: the `data-focus-scope` of the
		 * element that has to hold focus for it to act. When given, the shortcut
		 * only shows while that scope owns focus — a scoped hotkey does nothing
		 * from anywhere else, so advertising it there would mislead. Checked once
		 * as the popup mounts (it opens on hover, which doesn't move focus), so no
		 * subscription re-renders rows on pane switches.
		 */
		kbdScope?: string;
	}
> = ({ children, kbd, kbdScope, ...props }) => {
	const [kbdApplies] = useState(
		() =>
			kbdScope === undefined ||
			document.activeElement?.closest(`[data-focus-scope="${kbdScope}"]`) != null,
	);
	const showKbd = kbd != null && kbdApplies;

	return (
		<div
			{...props}
			className={classes(props.className, "text-12", styles.tooltip, showKbd && styles.withKbd)}
		>
			<span className={styles.content}>{children}</span>
			{showKbd && <Kbd hotkey={kbd} />}
		</div>
	);
};

/** @public */
export type TooltipProps = Omit<BaseTooltip.Root.Props, "children"> & {
	/** What the tooltip says. */
	content: ReactNode;
	/** The element it describes, usually a `Button`. It becomes the trigger, so it must take a ref and spread props. */
	children: ReactElement;
	/** A keyboard shortcut shown after the content. */
	kbd?: string | HotkeySequence;
	/** The focus scope the shortcut acts in; see `TooltipPopup`. */
	kbdScope?: string;
	side?: BaseTooltip.Positioner.Props["side"];
	sideOffset?: number;
};

/**
 * A tooltip on the element it wraps. An icon-only button still needs its own `aria-label`: a
 * tooltip shows on hover and focus, and a screen reader doesn't read it as the button's name.
 *
 * A disabled button swallows hover, so give one under a tooltip `focusableWhenDisabled`, which
 * keeps it hoverable and focusable while it can't be pressed.
 *
 * @import import { Tooltip } from "@gitbutler/ui-react/Tooltip.tsx";
 */
export const Tooltip: FC<TooltipProps> = ({
	content,
	children,
	kbd,
	kbdScope,
	side,
	sideOffset = 4,
	...rootProps
}) => (
	<BaseTooltip.Root {...rootProps}>
		<BaseTooltip.Trigger render={children} />
		<BaseTooltip.Portal>
			<BaseTooltip.Positioner side={side} sideOffset={sideOffset}>
				<BaseTooltip.Popup render={<TooltipPopup kbd={kbd} kbdScope={kbdScope} />}>
					{content}
				</BaseTooltip.Popup>
			</BaseTooltip.Positioner>
		</BaseTooltip.Portal>
	</BaseTooltip.Root>
);
