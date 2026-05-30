import { classes } from "#ui/components/classes.ts";
import styles from "./Tooltip.module.css";
import { Kbd } from "#ui/components/Kbd.tsx";
import { type HotkeySequence } from "@tanstack/react-hotkeys";
import { type ComponentProps, type FC, type ReactElement } from "react";

export const TooltipPopup: FC<
	Omit<ComponentProps<"div">, "content"> & {
		/** Content rendered inside the tooltip popup. */
		content?: ReactElement | string;
		/** Optional keyboard shortcut displayed alongside the content. */
		kbd?: string | HotkeySequence;
	}
> = ({ content = "", kbd, ...props }) => (
	<div {...props} className={classes(props.className, "text-12", styles.tooltip)}>
		{content !== "" && <span>{content}</span>}
		{kbd != null && <Kbd hotkey={kbd} />}
	</div>
);
