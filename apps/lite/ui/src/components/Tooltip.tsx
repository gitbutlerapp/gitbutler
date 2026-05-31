import { classes } from "#ui/components/classes.ts";
import styles from "./Tooltip.module.css";
import { Kbd } from "#ui/components/Kbd.tsx";
import { type HotkeySequence } from "@tanstack/react-hotkeys";
import { type ComponentProps, type FC } from "react";

export const TooltipPopup: FC<
	ComponentProps<"div"> & {
		/** Optional keyboard shortcut displayed alongside the content. */
		kbd?: string | HotkeySequence;
	}
> = ({ children, kbd, ...props }) => (
	<div {...props} className={classes(props.className, "text-12", styles.tooltip)}>
		<span>{children}</span>
		{kbd != null && <Kbd hotkey={kbd} />}
	</div>
);
