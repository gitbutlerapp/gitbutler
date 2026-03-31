import { classes } from "#ui/classes.ts";
import { mergeProps, Tooltip, useRender } from "@base-ui/react";
import { FC } from "react";
import { bindingButtonLabel, ShortcutActionBase, ShortcutBinding } from "#ui/shortcuts.ts";
import uiStyles from "#ui/ui.module.css";

type ShortcutButtonProps = {
	binding: ShortcutBinding<ShortcutActionBase>;
} & useRender.ComponentProps<"button">;

export const ShortcutButton: FC<ShortcutButtonProps> = ({ binding, render, ...props }) => {
	const tooltip = bindingButtonLabel(binding);
	const trigger = useRender({
		render,
		defaultTagName: "button",
		props: mergeProps<"button">({ "aria-label": tooltip }, props),
	});

	return (
		<Tooltip.Root
			// Prevent tooltip from continuing to show when mouse moves from one
			// selected item to another.
			// [tag:tooltip-disable-hoverable-popup]
			disableHoverablePopup
		>
			<Tooltip.Trigger render={trigger} />
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8}>
					<Tooltip.Popup className={classes(uiStyles.popup, uiStyles.tooltip)}>
						{tooltip}
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
