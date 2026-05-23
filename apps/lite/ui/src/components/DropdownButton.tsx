import { Button } from "#ui/components/Button.tsx";
import { ButtonGroup } from "#ui/components/ButtonGroup.tsx";
import { ShortcutButton } from "#ui/components/ShortcutButton.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { type TooltipPositionerProps } from "@base-ui/react";
import { type Hotkey, type UseHotkeyOptions } from "@tanstack/react-hotkeys";
import { type ComponentProps, type FC, type MouseEvent, type ReactNode } from "react";

type BaseButtonProps = Omit<ComponentProps<typeof Button>, "children">;

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
		ComponentProps<typeof Button>,
		"children" | "variant" | "size" | "disabled" | "type" | "onClick" | "aria-label"
	>;
};

function hasHotkey(hotkey: Hotkey | undefined): hotkey is Hotkey {
	return hotkey !== undefined;
}

export const DropdownButton: FC<Props> = ({
	children,
	variant,
	size,
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
		<Button
			{...primaryButtonProps}
			variant={variant}
			size={size}
			disabled={disabled}
			type={type}
			className={className}
			aria-label={resolvedPrimaryAriaLabel}
		>
			{children}
		</Button>
	);

	return (
		<ButtonGroup aria-label={groupAriaLabel}>
			{primary}
			<Button
				{...menuButtonProps}
				variant={variant}
				size={size}
				disabled={disabled}
				type="button"
				aria-label={menuAriaLabel}
				className={classes(menuButtonProps?.className)}
				onClick={onMenuOpen}
			>
				<Icon name="chevron-down" />
			</Button>
		</ButtonGroup>
	);
};
