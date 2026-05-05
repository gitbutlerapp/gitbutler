import { classes } from "#ui/ui/classes.ts";
import uiStyles from "#ui/ui/ui.module.css";
import { Tooltip } from "@base-ui/react";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import {
	formatForDisplay,
	useHotkey,
	type RegisterableHotkey,
	type UseHotkeyOptions,
} from "@tanstack/react-hotkeys";
import { ComponentProps, FC, useRef } from "react";

export const ShortcutButton: FC<
	Omit<ComponentProps<"button">, "children"> & {
		children?: string;
		hotkey: RegisterableHotkey;
		hotkeyOptions?: UseHotkeyOptions;
		/** @default `onClick` */
		onHotkey?: () => void;
	}
> = ({ children, hotkey, hotkeyOptions, onHotkey, ...props }) => {
	const buttonRef = useRef<HTMLButtonElement>(null);

	const handleHotkey = (): void => {
		if (onHotkey) onHotkey();
		else buttonRef.current?.click();
	};

	useHotkey(hotkey, handleHotkey, {
		...hotkeyOptions,
		enabled: !props.disabled && hotkeyOptions?.enabled !== false,
	});

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				{...props}
				ref={useMergedRefs(buttonRef, props.ref)}
				render={<button type="button" disabled={props.disabled} />}
			>
				{children}
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8}>
					<Tooltip.Popup className={classes(uiStyles.popup, uiStyles.tooltip)}>
						<kbd>{formatForDisplay(hotkey)}</kbd>
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
