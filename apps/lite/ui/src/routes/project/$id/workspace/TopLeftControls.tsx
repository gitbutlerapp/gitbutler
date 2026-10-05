import { Button } from "@gitbutler/ui-react/Button.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { sidebarFocusScopeOf } from "#ui/use-cursor.ts";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { Tooltip } from "@gitbutler/ui-react/Tooltip.tsx";
import { interfaceSlice } from "#ui/interface/state.ts";
import { focusScope } from "#ui/focus-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { workspaceHotkeys } from "#ui/hotkeys.ts";
import { useEffect, useState, type FC } from "react";
import styles from "./TopLeftControls.module.css";

const FullWindowButton: FC = () => {
	const dispatch = useAppDispatch();
	const fullWindow = useAppSelector(interfaceSlice.selectors.selectDetailsFullWindow);

	const toggle = () => {
		dispatch(interfaceSlice.actions.setDetailsFullWindow({ fullWindow: !fullWindow }));

		// Toggling swaps this button for the copy in the other pane, so the click leaves focus on
		// the body. Hand it to the pane the sidebar is folding out of, or back into.
		const sidebarFocusScope = sidebarFocusScopeOf();
		requestAnimationFrame(() => focusScope(fullWindow ? sidebarFocusScope : "diff"));
	};

	return (
		<Tooltip
			content={workspaceHotkeys.toggleSidebar.meta.name}
			kbd={workspaceHotkeys.toggleSidebar.hotkey}
		>
			<Button
				iconOnly
				variant="ghost"
				aria-label={workspaceHotkeys.toggleSidebar.meta.name}
				onClick={toggle}
			>
				{fullWindow ? <Icon name="sidebar-narrow" /> : <Icon name="sidebar" />}
			</Button>
		</Tooltip>
	);
};

const isMac = window.lite.platform === "darwin";

/**
 * Leaves room for the traffic lights, which are hidden in full-screen.
 *
 * Only mounted on macOS, so the full-screen subscription is set up there alone.
 */
const MacSpacer: FC = () => {
	const [fullScreen, setFullScreen] = useState(false);

	useEffect(() => {
		let notified = false;
		const unsubscribe = window.lite.onFullScreenChange((value) => {
			notified = true;
			setFullScreen(value);
		});
		// An event received while the query is in flight is more recent than its result.
		void window.lite.isFullScreen().then((value) => {
			if (!notified) setFullScreen(value);
		});
		return unsubscribe;
	}, []);

	return fullScreen ? null : <div className={styles.macSpacer} />;
};

/**
 * The window's controls, which stay put beside the traffic lights as the sidebar folds: the
 * sidebar's header holds them, then the details header or, in the Uncommitted view, the diff's
 * bar. `placement` names the container, so they undo its padding and land where the sidebar has them.
 */
export const TopLeftControls: FC<{ placement?: "sidebar" | "viewHeader" | "diffBar" }> = ({
	placement = "sidebar",
}) => (
	<div
		className={classes(
			styles.container,
			placement === "viewHeader" && styles.inViewHeader,
			placement === "diffBar" && styles.inDiffBar,
		)}
	>
		{isMac && <MacSpacer />}
		<FullWindowButton />
	</div>
);
