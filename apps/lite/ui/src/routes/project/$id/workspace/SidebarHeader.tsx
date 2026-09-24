import { Button } from "@gitbutler/ui-react/Button.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { Tooltip } from "@gitbutler/ui-react/Tooltip.tsx";
import { globalHotkeys, workspaceHotkeys } from "#ui/hotkeys.ts";
import { ProjectPicker } from "#ui/routes/project/$id/workspace/ProjectPicker.tsx";
import { TopLeftControls } from "#ui/routes/project/$id/workspace/TopLeftControls.tsx";
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
 * picker, activity, the operations log and settings. Purely presentational.
 */
export const SidebarHeader: FC<{
	project: ProjectForFrontend;
	/** A fetch in flight shows its own spinner on the target's row, so the ambient one stands down. */
	isFetchPending: boolean;
	canOpenOperationsLog: boolean;
	onOpenOperationsLog: () => void;
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
			<Tooltip
				content={globalHotkeys.operationsLog.meta.name}
				kbd={globalHotkeys.operationsLog.hotkey}
			>
				<Button
					iconOnly
					variant="ghost"
					focusableWhenDisabled
					disabled={!p.canOpenOperationsLog}
					aria-label={globalHotkeys.operationsLog.meta.name}
					onClick={p.onOpenOperationsLog}
				>
					<Icon name="history" />
				</Button>
			</Tooltip>

			<Tooltip content={workspaceHotkeys.settings.meta.name} kbd={workspaceHotkeys.settings.hotkey}>
				<Button
					iconOnly
					variant="ghost"
					focusableWhenDisabled
					disabled={!p.canOpenSettings}
					aria-label={workspaceHotkeys.settings.meta.name}
					onClick={p.onOpenSettings}
				>
					<Icon name="settings" />
				</Button>
			</Tooltip>
			{p.bell}
		</div>
	</header>
);
