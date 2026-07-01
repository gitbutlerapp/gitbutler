import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { projectActions, selectProjectDetailsFullWindow } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { workspaceHotkeys } from "#ui/hotkeys.ts";
import { Toggle, Tooltip } from "@base-ui/react";
import { useParams } from "@tanstack/react-router";
import { type ComponentProps, type FC } from "react";
import styles from "./TopLeftControls.module.css";

const FullWindowToggle: FC<
	Omit<ComponentProps<typeof Toggle>, "aria-label" | "pressed" | "onPressedChange">
> = (toggleProps) => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const dispatch = useAppDispatch();
	const fullWindow = useAppSelector((state) => selectProjectDetailsFullWindow(state, projectId));

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				render={
					<Toggle
						{...toggleProps}
						aria-label={workspaceHotkeys.toggleOutline.meta.name}
						pressed={fullWindow}
						onPressedChange={(fullWindow) =>
							dispatch(projectActions.setDetailsFullWindow({ projectId, fullWindow }))
						}
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
};

const isMac = window.lite.platform === "darwin";

export const TopLeftControls: FC = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const fullWindow = useAppSelector((state) => selectProjectDetailsFullWindow(state, projectId));
	return (
		<div className={styles.container}>
			{isMac && <div className={styles.macSpacer} />}
			<FullWindowToggle className={getButtonClassName({ iconOnly: true, variant: "ghost" })}>
				{fullWindow ? <Icon name="sidebar-show" /> : <Icon name="sidebar-hide" />}
			</FullWindowToggle>
		</div>
	);
};
