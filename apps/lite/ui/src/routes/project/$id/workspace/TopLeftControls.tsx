import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { interfaceSlice } from "#ui/interface/state.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { focusSelectionScope } from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import { workspaceHotkeys } from "#ui/hotkeys.ts";
import { Tooltip } from "@base-ui/react";
import { useParams } from "@tanstack/react-router";
import { useEffect, useState, type FC } from "react";
import styles from "./TopLeftControls.module.css";

const FullWindowButton: FC = () => {
	const dispatch = useAppDispatch();
	const store = useAppStore();
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const fullWindow = useAppSelector(interfaceSlice.selectors.selectDetailsFullWindow);

	const toggle = () => {
		dispatch(interfaceSlice.actions.setDetailsFullWindow({ fullWindow: !fullWindow }));

		// Toggling swaps this button for the copy in the other pane, so the click leaves focus on
		// the body. Hand it to the pane the outline is folding out of, or back into.
		const detailsSelectionScope = projectSlice.selectors.selectDetailsSelectionScope(
			store.getState(),
			projectId,
		);
		requestAnimationFrame(() =>
			focusSelectionScope(fullWindow ? (detailsSelectionScope ?? "outline") : "diff"),
		);
	};

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				render={
					<button
						type="button"
						className={getButtonClassName({ iconOnly: true, variant: "ghost" })}
						aria-label={workspaceHotkeys.toggleOutline.meta.name}
						onClick={toggle}
					>
						{fullWindow ? <Icon name="sidebar-narrow" /> : <Icon name="sidebar" />}
					</button>
				}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup kbd={workspaceHotkeys.toggleOutline.hotkey} />}>
						{workspaceHotkeys.toggleOutline.meta.name}
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
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

export const TopLeftControls: FC = () => (
	<div className={styles.container}>
		{isMac && <MacSpacer />}
		<FullWindowButton />
	</div>
);
