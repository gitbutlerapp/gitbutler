import styles from "./ShortcutButton.module.css";
import { Keys } from "#ui/components/Keys.tsx";
import { classes } from "#ui/ui/classes.ts";
import uiStyles from "#ui/ui/ui.module.css";
import { Tooltip, TooltipPositionerProps } from "@base-ui/react";
import { type Hotkey, type UseHotkeyOptions } from "@tanstack/react-hotkeys";
import { ComponentProps, FC } from "react";

type Props = ComponentProps<"button"> & {
	hotkey: Hotkey;
	hotkeyOptions?: UseHotkeyOptions;
	positionerProps?: TooltipPositionerProps;
};

export const ShortcutButton: FC<Props> = ({ hotkey, hotkeyOptions, positionerProps, ...props }) => {
	const hotkeyEnabled = !props.disabled && hotkeyOptions?.enabled !== false;

	return (
		<Tooltip.Root disabled={!hotkeyEnabled}>
			<Tooltip.Trigger
				{...props}
				// This is needed to ensure the `disabled` attribute is used.
				render={
					<button
						disabled={props.disabled}
						// Preserve default behaviour of `Tooltip.Trigger` without a custom `render`.
						// oxlint-disable-next-line react/button-has-type -- False positive.
						type={props.type ?? "button"}
					/>
				}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8} {...positionerProps}>
					<Tooltip.Popup className={classes(uiStyles.popup, uiStyles.tooltip, styles.tooltip)}>
						{hotkeyOptions?.meta?.name}
						<Keys hotkey={hotkey} />
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
