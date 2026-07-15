import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { useProjectStore } from "#ui/store.ts";
import { workspaceHotkeys } from "#ui/hotkeys.ts";
import { Toggle, Tooltip } from "@base-ui/react";
import { useParams } from "@tanstack/react-router";
import { type ComponentProps, type FC } from "react";
import styles from "./TopLeftControls.module.css";
import { observer } from "mobx-react-lite";

const FullWindowToggle: FC<
	Omit<ComponentProps<typeof Toggle>, "aria-label" | "pressed" | "onPressedChange">
> = observer((toggleProps) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const projectStore = useProjectStore(projectId);
	const fullWindow = projectStore.detailsFullWindow;

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				render={
					<Toggle
						{...toggleProps}
						aria-label={workspaceHotkeys.toggleOutline.meta.name}
						pressed={fullWindow}
						onPressedChange={(fullWindow) => projectStore.setDetailsFullWindow(fullWindow)}
					/>
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
});

const isMac = window.lite.platform === "darwin";

export const TopLeftControls: FC = observer(() => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const fullWindow = useProjectStore(projectId).detailsFullWindow;
	return (
		<div className={styles.container}>
			{isMac && <div className={styles.macSpacer} />}
			<FullWindowToggle className={getButtonClassName({ iconOnly: true, variant: "ghost" })}>
				{fullWindow ? <Icon name="sidebar-show" /> : <Icon name="sidebar-hide" />}
			</FullWindowToggle>
		</div>
	);
});
