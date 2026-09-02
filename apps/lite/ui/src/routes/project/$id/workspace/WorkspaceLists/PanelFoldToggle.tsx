import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import type { SidebarPanel } from "#ui/projects/project.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { Button, Tooltip } from "@base-ui/react";
import type { FC } from "react";
import styles from "./PanelFoldToggle.module.css";

const label: Record<SidebarPanel, string> = {
	uncommitted: "uncommitted changes",
	stacks: "stacks and branches",
};

/**
 * Collapses one of the sidebar's two stacked panels to its header, so the other
 * has the tree to itself.
 *
 * Never disabled: the store keeps one panel open, so collapsing the open one
 * while the other is already collapsed swaps them rather than doing nothing.
 * Chevrons rather than the panel-collapse glyphs, because folding a section is
 * a gesture this sidebar already has on its branch rows.
 */
export const PanelFoldToggle: FC<{ projectId: string; panel: SidebarPanel }> = ({
	projectId,
	panel,
}) => {
	const dispatch = useAppDispatch();
	const collapsed = useAppSelector((state) =>
		projectSlice.selectors.selectSidebarPanelCollapsed(state, projectId, panel),
	);
	const action = `${collapsed ? "Expand" : "Collapse"} ${label[panel]}`;

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				aria-label={action}
				aria-expanded={!collapsed}
				className={styles.toggle}
				onClick={() =>
					dispatch(projectSlice.actions.toggleSidebarPanelCollapsed({ projectId, panel }))
				}
				render={<Button />}
			>
				<Icon size={14} name={collapsed ? "chevron-right" : "chevron-down"} />
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>{action}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
