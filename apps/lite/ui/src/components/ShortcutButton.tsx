import { getButtonClassName, type ButtonStyleProps } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Tooltip } from "#ui/components/Tooltip.tsx";
import { TooltipPositionerProps } from "@base-ui/react";
import { type Hotkey, type UseHotkeyOptions } from "@tanstack/react-hotkeys";
import { type ComponentProps, type FC } from "react";

type Props = ComponentProps<"button"> &
	ButtonStyleProps & {
		hotkey: Hotkey;
		hotkeyOptions?: UseHotkeyOptions;
		positionerProps?: TooltipPositionerProps;
	};

export const ShortcutButton: FC<Props> = ({
	hotkey,
	hotkeyOptions,
	positionerProps,
	variant,
	size,
	iconOnly,
	className,
	type = "button",
	...props
}) => {
	const hotkeyEnabled = !props.disabled && hotkeyOptions?.enabled !== false;

	return (
		<Tooltip
			disabled={!hotkeyEnabled}
			// This is needed to ensure the `disabled` attribute is used.
			trigger={
				<button
					{...props}
					// oxlint-disable-next-line react/button-has-type -- False positive.
					type={type}
					className={classes(getButtonClassName({ variant, size, iconOnly }), className)}
				/>
			}
			content={hotkeyOptions?.meta?.name}
			kbd={hotkey}
			positionerProps={positionerProps}
		/>
	);
};
