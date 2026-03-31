import { Tooltip, useRender } from "@base-ui/react";
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
		props,
	});

	return (
		<Tooltip.Root>
			<Tooltip.Trigger render={trigger} aria-label={tooltip} />
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8}>
					<Tooltip.Popup className={uiStyles.tooltip}>{tooltip}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
