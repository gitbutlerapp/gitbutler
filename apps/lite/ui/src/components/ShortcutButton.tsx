import { Button } from "#ui/components/Button.tsx";
import { Tooltip } from "#ui/components/Tooltip.tsx";
import { TooltipPositionerProps } from "@base-ui/react";
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
		<Tooltip
			disabled={!hotkeyEnabled}
			// This is needed to ensure the `disabled` attribute is used.
			trigger={<Button {...props} disabled={props.disabled} type={props.type} />}
			content={hotkeyOptions?.meta?.name}
			kbd={hotkey}
			positionerProps={positionerProps}
		/>
	);
};
