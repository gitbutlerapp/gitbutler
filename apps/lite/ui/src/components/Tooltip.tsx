import { classes } from "#ui/components/classes.ts";
import styles from "./Tooltip.module.css";
import { Kbd } from "#ui/components/Kbd.tsx";
import { Tooltip as BaseTooltip, TooltipPositionerProps } from "@base-ui/react";
import { HotkeySequence } from "@tanstack/react-hotkeys";
import { ComponentProps, ReactElement } from "react";

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
					<BaseTooltip.Popup className={styles.tooltip}>
						{content !== "" && <span className={classes("text-12", styles.text)}>{content}</span>}
						{kbd != null && <Kbd hotkey={kbd} />}
					</BaseTooltip.Popup>
				</BaseTooltip.Positioner>
			</BaseTooltip.Portal>
		</BaseTooltip.Root>
	);
}
