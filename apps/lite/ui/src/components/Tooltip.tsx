import { classes } from "#ui/components/classes.ts";
import styles from "./Tooltip.module.css";
import { Kbd } from "#ui/components/Kbd.tsx";
import { isFocusWithinSelectionScope, type SelectionScope } from "#ui/selection-scopes.ts";
import type { HotkeySequence } from "@tanstack/react-hotkeys";
import { useState, type ComponentProps, type FC } from "react";

export const TooltipPopup: FC<
	ComponentProps<"div"> & {
		/** Optional keyboard shortcut displayed alongside the content. */
		kbd?: string | HotkeySequence;
		/**
		 * The selection scope the shortcut is bound to. When given, the shortcut
		 * only shows while that scope owns focus — a scoped hotkey does nothing
		 * from anywhere else, so advertising it there would mislead. Checked once
		 * as the popup mounts (it opens on hover, which doesn't move focus), so no
		 * subscription re-renders rows on pane switches.
		 */
		kbdScope?: SelectionScope;
	}
> = ({ children, kbd, kbdScope, ...props }) => {
	const [kbdApplies] = useState(
		() => kbdScope === undefined || isFocusWithinSelectionScope(kbdScope),
	);
	const showKbd = kbd != null && kbdApplies;

	return (
		<div
			{...props}
			className={classes(props.className, "text-12", styles.tooltip, showKbd && styles.withKbd)}
		>
			<span className={styles.content}>{children}</span>
			{showKbd && <Kbd hotkey={kbd} />}
		</div>
	);
};
