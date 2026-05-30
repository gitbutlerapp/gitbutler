import { getButtonClassName, type ButtonStyleProps } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { Tooltip, TooltipPositionerProps } from "@base-ui/react";
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
		<Tooltip.Root disabled={!hotkeyEnabled}>
			<Tooltip.Trigger
				// This is needed to ensure the `disabled` attribute is used.
				render={
					<button
						{...props}
						// oxlint-disable-next-line react/button-has-type -- False positive.
						type={type}
						className={classes(getButtonClassName({ variant, size, iconOnly }), className)}
					/>
				}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4} {...positionerProps}>
					<Tooltip.Popup
						render={<TooltipPopup content={hotkeyOptions?.meta?.name} kbd={hotkey} />}
					/>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
