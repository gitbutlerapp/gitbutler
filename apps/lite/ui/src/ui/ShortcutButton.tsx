import styles from "./ShortcutButton.module.css";
import { Keys } from "#ui/ui/Keys.tsx";
import { classes } from "#ui/ui/classes.ts";
import uiStyles from "#ui/ui/ui.module.css";
import { Tooltip } from "@base-ui/react";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import type { HotkeySequence, Hotkey } from "@tanstack/react-hotkeys";
import { ComponentProps, FC, useRef } from "react";

export const ShortcutButton: FC<
	ComponentProps<"button"> & {
		hotkeys?: Array<Hotkey | HotkeySequence>;
	}
> = ({ hotkeys, ...props }) => {
	const buttonRef = useRef<HTMLButtonElement>(null);

	// TODO: Render all hotkeys.
	const firstViable = hotkeys?.find((hk) => typeof hk === "string");

	return (
		<Tooltip.Root disabled={props.disabled || firstViable === undefined}>
			<Tooltip.Trigger
				{...props}
				ref={useMergedRefs(buttonRef, props.ref)}
				// This is needed to ensure the `disabled` attribute is used.
				render={<button type="button" disabled={props.disabled} />}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8}>
					<Tooltip.Popup className={classes(uiStyles.popup, uiStyles.tooltip, styles.tooltip)}>
						{firstViable !== undefined && <Keys hotkey={firstViable} />}
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
