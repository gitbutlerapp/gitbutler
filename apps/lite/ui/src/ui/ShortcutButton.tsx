import styles from "./ShortcutButton.module.css";
import { Keys } from "#ui/ui/Keys.tsx";
import { classes } from "#ui/ui/classes.ts";
import uiStyles from "#ui/ui/ui.module.css";
import { Tooltip } from "@base-ui/react";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { useHotkey, type RegisterableHotkey, type UseHotkeyOptions } from "@tanstack/react-hotkeys";
import { ComponentProps, FC, useRef } from "react";

export const ShortcutButton: FC<
	ComponentProps<"button"> & {
		hotkey: RegisterableHotkey;
		hotkeyOptions?: UseHotkeyOptions;
	}
> = ({ hotkey, hotkeyOptions, ...props }) => {
	const buttonRef = useRef<HTMLButtonElement>(null);

	const hotkeyEnabled = !props.disabled && hotkeyOptions?.enabled !== false;

	useHotkey(hotkey, () => buttonRef.current?.click(), {
		...hotkeyOptions,
		enabled: hotkeyEnabled,
	});

	return (
		<Tooltip.Root disabled={!hotkeyEnabled}>
			<Tooltip.Trigger
				{...props}
				ref={useMergedRefs(buttonRef, props.ref)}
				// This is needed to ensure the `disabled` attribute is used.
				render={<button type="button" disabled={props.disabled} />}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8}>
					<Tooltip.Popup className={classes(uiStyles.popup, uiStyles.tooltip, styles.tooltip)}>
						{hotkeyOptions?.meta?.name}
						<Keys hotkey={hotkey} />
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
