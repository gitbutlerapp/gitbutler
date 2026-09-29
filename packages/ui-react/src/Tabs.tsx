import { Tabs as BaseTabs } from "@base-ui/react";
import type { HotkeySequence } from "@tanstack/react-hotkeys";
import type { FC, ReactNode } from "react";
import { Badge } from "./Badge.tsx";
import { classes } from "./classes.ts";
import { Kbd } from "./Kbd.tsx";
import styles from "./Tabs.module.css";

/**
 * A row of {@link Tab}s that switch what a pane shows, one at a time, with an underline under the
 * selected one that slides to the next as the selection moves.
 *
 * - Controlled with `value` and `onValueChange`, or left to itself with `defaultValue`.
 * - The arrow keys move between the tabs, as a tab list does.
 * - The underline runs under the selected tab's content and hangs outside the row, 8px below it
 *   and 4px thick, so the header holding the tabs leaves 12px below them: the underline then
 *   sits on the header's divider.
 *
 * @public
 * @import import { Tabs, Tab } from "@gitbutler/ui-react/Tabs.tsx";
 */
export const Tabs: FC<
	Omit<BaseTabs.Root.Props, "className"> & {
		className?: string;
		"aria-label"?: string;
	}
> = ({ className, children, "aria-label": ariaLabel, ...props }) => (
	<BaseTabs.Root {...props} className={className}>
		<BaseTabs.List className={styles.list} aria-label={ariaLabel}>
			{children}
			<BaseTabs.Indicator className={styles.indicator} />
		</BaseTabs.List>
	</BaseTabs.Root>
);

/**
 * One tab in {@link Tabs}: its label, an optional `icon` before it and an optional `count` after
 * it, for how many things the tab holds. `kbd` names the shortcut that selects it, faint after
 * the label. Faded until it is hovered or selected.
 *
 * @public
 */
export const Tab: FC<
	Omit<BaseTabs.Tab.Props, "className"> & {
		className?: string;
		icon?: ReactNode;
		count?: number;
		kbd?: string | HotkeySequence;
	}
> = ({ className, icon, count, kbd, children, ...props }) => (
	<BaseTabs.Tab {...props} className={classes(className, "text-13", "text-semibold", styles.tab)}>
		{icon !== undefined && <span className={styles.icon}>{icon}</span>}
		<span className={styles.label}>
			{children}
			{kbd !== undefined && <Kbd hotkey={kbd} variant="button" />}
		</span>
		{count !== undefined && <Badge variant="lightGray">{count}</Badge>}
	</BaseTabs.Tab>
);
