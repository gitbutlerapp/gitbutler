import { classes } from "#ui/components/classes.ts";
import styles from "./Tooltip.module.css";
import { Kbd } from "#ui/components/Kbd.tsx";
import { Tooltip as BaseTooltip, TooltipPositionerProps } from "@base-ui/react";
import { HotkeySequence } from "@tanstack/react-hotkeys";
import React, { ComponentProps, FC, ReactElement } from "react";

const TooltipPopup: FC<
	Omit<React.ComponentProps<"div">, "content"> & {
		/** Content rendered inside the tooltip popup. */
		content?: ReactElement | string;
		/** Optional keyboard shortcut displayed alongside the content. */
		kbd?: string | HotkeySequence;
	}
> = ({ content = "", kbd, ...props }) => (
	<div {...props} className={classes(props.className, styles.tooltip)}>
		{content !== "" && <span className={classes("text-12", styles.text)}>{content}</span>}
		{kbd != null && <Kbd hotkey={kbd} />}
	</div>
);

type TooltipProps = Omit<ComponentProps<typeof BaseTooltip.Root>, "children"> & {
	/** The trigger element — rendered inside `Tooltip.Trigger` via the `render` prop. */
	trigger: ReactElement;
	/** Content rendered inside the tooltip popup. */
	content?: ReactElement | string;
	/** Optional keyboard shortcut displayed alongside the content. */
	kbd?: string | HotkeySequence;
	positionerProps?: TooltipPositionerProps;
};

export function Tooltip({
	trigger,
	content = "",
	kbd,
	positionerProps,
	...rootProps
}: TooltipProps) {
	return (
		<BaseTooltip.Root {...rootProps}>
			<BaseTooltip.Trigger render={trigger} />
			<BaseTooltip.Portal>
				<BaseTooltip.Positioner sideOffset={4} {...positionerProps}>
					<BaseTooltip.Popup render={<TooltipPopup content={content} kbd={kbd} />} />
				</BaseTooltip.Positioner>
			</BaseTooltip.Portal>
		</BaseTooltip.Root>
	);
}
