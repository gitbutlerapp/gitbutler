import styles from "./ShortcutButton.module.css";
import { Button } from "#ui/components/Button.tsx";
import { Keys } from "#ui/components/Keys.tsx";
import { classes } from "#ui/components/classes.ts";
import uiStyles from "#ui/components/ui.module.css";
import { Tooltip, TooltipPositionerProps } from "@base-ui/react";
import { type Hotkey, type UseHotkeyOptions } from "@tanstack/react-hotkeys";
import { ComponentProps, FC } from "react";

type Props = ComponentProps<typeof Button> & {
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
				render={<Button disabled={props.disabled} type={props.type} />}
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
