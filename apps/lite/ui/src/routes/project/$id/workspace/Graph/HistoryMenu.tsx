import { useSaveGUISettings } from "#ui/api/mutations.ts";
import { guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { defaultSettings } from "#ui/settings.ts";
import { nativeMenuItem, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { useQuery } from "@tanstack/react-query";
import type { GUISettings } from "#electron/settings.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { getRowButtonClassName } from "#ui/routes/project/$id/workspace/Row-utils.ts";
import { Button, Tooltip } from "@base-ui/react";
import type { FC } from "react";

export const HistoryMenu: FC<{ projectId: string }> = ({ projectId }) => {
	const dispatch = useAppDispatch();
	const { data: mode = defaultSettings.historyDisplayMode } = useQuery({
		...guiSettingsQueryOptions,
		select: (settings) => settings.historyDisplayMode ?? defaultSettings.historyDisplayMode,
	});
	const { mutate: saveGUISettings } = useSaveGUISettings();
	const select = (historyDisplayMode: NonNullable<GUISettings["historyDisplayMode"]>) => {
		if (historyDisplayMode === mode) return;
		dispatch(projectSlice.actions.resetGraphHistory({ projectId }));
		saveGUISettings({ historyDisplayMode });
	};
	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				aria-label="History menu"
				aria-haspopup="menu"
				className={getRowButtonClassName({ iconOnly: true })}
				render={<Button />}
				onClick={(event) => {
					void showNativeMenuFromTrigger(event.currentTarget, [
						nativeMenuItem({
							label: "Last 5 commits",
							checked: mode === "commits",
							onSelect: () => select("commits"),
						}),
						nativeMenuItem({
							label: "Last 12 hours",
							checked: mode === "last-12-hours",
							onSelect: () => select("last-12-hours"),
						}),
					]);
				}}
			>
				<Icon name="kebab" />
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>History menu</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
