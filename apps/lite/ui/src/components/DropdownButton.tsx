import { getButtonClassName, type ButtonStyleProps } from "#ui/components/Button.tsx";
import { ButtonGroup } from "#ui/components/ButtonGroup.tsx";
import { ShortcutButton } from "#ui/components/ShortcutButton.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { type TooltipPositionerProps } from "@base-ui/react";
import { type Hotkey, type UseHotkeyOptions } from "@tanstack/react-hotkeys";
import { type ComponentProps, type FC, type MouseEvent, type ReactNode } from "react";

type BaseButtonProps = Omit<ComponentProps<"button">, "children"> & ButtonStyleProps;

type Props = BaseButtonProps & {
	children: ReactNode;
	/** When provided, wraps the primary button in a hotkey tooltip. */
	hotkey?: Hotkey;
	hotkeyOptions?: UseHotkeyOptions;
	positionerProps?: TooltipPositionerProps;
	/** Optional explicit aria-label for the primary action button. */
	primaryAriaLabel?: string;
	/** Accessible name for the ButtonGroup wrapper. */
	groupAriaLabel?: string;
	/** Called when the chevron/dropdown trigger is clicked. */
	onMenuOpen: (event: MouseEvent<HTMLButtonElement>) => void;
	menuAriaLabel?: string;
	menuButtonProps?: Omit<
		ComponentProps<"button"> & ButtonStyleProps,
		"children" | "variant" | "size" | "iconOnly" | "disabled" | "type" | "onClick" | "aria-label"
	>;
};

function hasHotkey(hotkey: Hotkey | undefined): hotkey is Hotkey {
	return hotkey !== undefined;
}

export const DropdownButton: FC<Props> = ({
	children,
	variant,
	size,
	iconOnly,
	disabled,
	type,
	className,
	hotkey,
	hotkeyOptions,
	positionerProps,
	primaryAriaLabel,
	groupAriaLabel,
	onMenuOpen,
	menuAriaLabel = "More options",
	menuButtonProps,
	"aria-label": ariaLabel,
	...primaryButtonProps
}) => {
	const resolvedPrimaryAriaLabel = primaryAriaLabel ?? ariaLabel;

	const primary = hasHotkey(hotkey) ? (
		<ShortcutButton
			{...primaryButtonProps}
			variant={variant}
			size={size}
			iconOnly={iconOnly}
			disabled={disabled}
			type={type}
			className={className}
			aria-label={resolvedPrimaryAriaLabel}
			hotkey={hotkey}
			hotkeyOptions={hotkeyOptions}
			positionerProps={positionerProps}
		>
			{children}
		</ShortcutButton>
	) : (
		<button
			{...primaryButtonProps}
			disabled={disabled}
			// oxlint-disable-next-line react/button-has-type -- False positive.
			type={type ?? "button"}
			className={classes(getButtonClassName({ variant, size, iconOnly }), className)}
			aria-label={resolvedPrimaryAriaLabel}
		>
			{children}
		</button>
	);

	return (
		<ButtonGroup aria-label={groupAriaLabel}>
			{primary}
			<button
				{...menuButtonProps}
				disabled={disabled}
				type="button"
				aria-label={menuAriaLabel}
				className={classes(
					getButtonClassName({ variant, size, iconOnly: true }),
					menuButtonProps?.className,
				)}
				onClick={onMenuOpen}
			>
				<Icon name="chevron-down" />
			</button>
		</ButtonGroup>
	);
};
