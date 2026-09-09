import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { workspaceHotkeys } from "#ui/hotkeys.ts";
import { ProjectPicker } from "#ui/routes/project/$id/workspace/ProjectPicker.tsx";
import { TopLeftControls } from "#ui/routes/project/$id/workspace/TopLeftControls.tsx";
import { Button, Tooltip } from "@base-ui/react";
import type { ProjectForFrontend } from "@gitbutler/but-sdk";
import { useIsFetching, useIsMutating } from "@tanstack/react-query";
import { Match } from "effect";
import type { FC, ReactNode } from "react";
import styles from "./SidebarHeader.module.css";

const ActivitySpinner: FC<{
	/** Suppressed while the fetch button shows its own spinner, to avoid two spinners at once. */
	suppressed: boolean;
}> = (p) => {
	const fetchingCount = useIsFetching();
	const mutatingCount = useIsMutating();

	const isFetching = fetchingCount > 0;
	const isMutating = mutatingCount > 0;

	const status = Match.value({ isFetching, isMutating }).pipe(
		Match.when({ isFetching: true, isMutating: true }, () => "Syncing"),
		Match.when({ isFetching: true }, () => "Loading"),
		Match.when({ isMutating: true }, () => "Saving"),
		Match.orElse(() => null),
	);

	return (
		!p.suppressed &&
		status !== null && (
			<Icon name="spinner" aria-label={status} className={styles.activitySpinner} />
		)
	);
};

/**
 * The app chrome at the top of the sidebar: window controls, the project
 * picker, activity and settings. Purely presentational.
 */
export const SidebarHeader: FC<{
	project: ProjectForFrontend;
	/** A fetch in flight shows its own spinner on the target's row, so the ambient one stands down. */
	isFetchPending: boolean;
	canOpenSettings: boolean;
	onOpenSettings: () => void;
	/** The notification bell, which decides its own visibility. */
	bell?: ReactNode;
}> = (p) => (
	<header className={styles.workspaceControls}>
		<TopLeftControls />

		<div className={styles.workspaceControlsLeft}>
			<ProjectPicker project={p.project} />
			<ActivitySpinner suppressed={p.isFetchPending} />
		</div>

		<div className={styles.workspaceControlsActions}>
			<Tooltip.Root>
				<Tooltip.Trigger
					aria-label={workspaceHotkeys.settings.meta.name}
					className={getButtonClassName({ iconOnly: true, variant: "ghost" })}
					onClick={p.onOpenSettings}
					// We pass `disabled` here because we want to disable the button, not
					// the tooltip. Other props should be passed above.
					render={<Button focusableWhenDisabled disabled={!p.canOpenSettings} />}
				>
					<Icon name="settings" />
				</Tooltip.Trigger>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
						<Tooltip.Popup render={<TooltipPopup kbd={workspaceHotkeys.settings.hotkey} />}>
							{workspaceHotkeys.settings.meta.name}
						</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>
			{p.bell}
		</div>
	</header>
);
