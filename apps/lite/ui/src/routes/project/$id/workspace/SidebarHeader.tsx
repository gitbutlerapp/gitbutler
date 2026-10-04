import { Button } from "@gitbutler/ui-react/Button.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import type { IconName } from "@gitbutler/ui-react/iconNames.ts";
import { Tooltip } from "@gitbutler/ui-react/Tooltip.tsx";
import { globalHotkeys, workspaceHotkeys } from "#ui/hotkeys.ts";
import { TopLeftControls } from "#ui/routes/project/$id/workspace/TopLeftControls.tsx";
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

/** An icon button with its name as a tooltip, as each of the header's actions is. */
const HeaderButton: FC<{
	name: string;
	icon: IconName;
	kbd?: string;
	disabled?: boolean;
	onClick: () => void;
}> = (p) => (
	<Tooltip content={p.name} kbd={p.kbd}>
		<Button
			iconOnly
			variant="ghost"
			focusableWhenDisabled
			disabled={p.disabled}
			aria-label={p.name}
			onClick={p.onClick}
		>
			<Icon name={p.icon} />
		</Button>
	</Tooltip>
);

/**
 * The app chrome at the top of the sidebar: window controls, activity, and the actions that apply
 * where it runs; each is left out without its handler. Purely presentational.
 */
export const SidebarHeader: FC<{
	/** A fetch in flight shows its own spinner on the target's row, so the ambient one stands down. */
	isFetchPending: boolean;
	onAddRepository?: () => void;
	isAddingRepository?: boolean;
	canOpenOperationsLog: boolean;
	onOpenOperationsLog?: () => void;
	canOpenSettings: boolean;
	onOpenSettings?: () => void;
	onSignOut?: () => void;
	/** The notification bell, which decides its own visibility. */
	bell?: ReactNode;
}> = (p) => (
	<header className={styles.workspaceControls}>
		<TopLeftControls />

		<div className={styles.workspaceControlsLeft}>
			<ActivitySpinner suppressed={p.isFetchPending} />
		</div>

		<div className={styles.workspaceControlsActions}>
			{p.onAddRepository && (
				<HeaderButton
					name="Add local repository"
					icon="plus"
					disabled={p.isAddingRepository}
					onClick={p.onAddRepository}
				/>
			)}
			{p.onOpenOperationsLog && (
				<HeaderButton
					name={globalHotkeys.operationsLog.meta.name}
					icon="history"
					kbd={globalHotkeys.operationsLog.hotkey}
					disabled={!p.canOpenOperationsLog}
					onClick={p.onOpenOperationsLog}
				/>
			)}
			{p.onOpenSettings && (
				<HeaderButton
					name={workspaceHotkeys.settings.meta.name}
					icon="settings"
					kbd={workspaceHotkeys.settings.hotkey}
					disabled={!p.canOpenSettings}
					onClick={p.onOpenSettings}
				/>
			)}
			{p.onSignOut && <HeaderButton name="Sign out" icon="logout" onClick={p.onSignOut} />}
			{p.bell}
		</div>
	</header>
);
