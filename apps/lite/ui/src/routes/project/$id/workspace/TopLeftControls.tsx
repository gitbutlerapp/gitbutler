import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { DetailsFullWindowContext } from "#ui/DetailsFullWindowContext.ts";
import { workspaceHotkeys } from "#ui/hotkeys.ts";
import { Toggle, Tooltip } from "@base-ui/react";
import { type ComponentProps, type FC, use } from "react";
import styles from "./TopLeftControls.module.css";

const FullWindowToggle: FC<
	Omit<ComponentProps<typeof Toggle>, "aria-label" | "pressed" | "onPressedChange">
> = (toggleProps) => {
	const { detailsFullWindow, setDetailsFullWindow } = use(DetailsFullWindowContext);

	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				render={
					<Toggle
						{...toggleProps}
						aria-label={workspaceHotkeys.toggleOutline.meta.name}
						pressed={detailsFullWindow}
						onPressedChange={setDetailsFullWindow}
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
	const { detailsFullWindow } = use(DetailsFullWindowContext);
	return (
		<div className={styles.container}>
			{isMac && <div className={styles.macSpacer} />}
			<FullWindowToggle className={getButtonClassName({ iconOnly: true, variant: "ghost" })}>
				{detailsFullWindow ? <Icon name="sidebar-show" /> : <Icon name="sidebar-hide" />}
			</FullWindowToggle>
		</div>
	);
};
