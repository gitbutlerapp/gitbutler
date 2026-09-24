import { useSuspenseQuery } from "@tanstack/react-query";
import type { FC, ReactNode } from "react";
import { appSettingsQueryOptions, worktreesListQueryOptions } from "#ui/api/queries.ts";
import { useWorktreeRemove, useWorktreeSetArchived } from "#ui/api/mutations.ts";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { branchDetailsParams } from "#ui/branch.ts";
import { revealInFolderLabel } from "#ui/hotkeys.ts";
import { nativeMenuItem, nativeMenuSeparator, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import type { ListedWorktree } from "@gitbutler/but-sdk";
import { IconButton } from "./IconButton.tsx";
import styles from "./Worktrees.module.css";
import { Section } from "./Section.tsx";

/** A card with nothing to list: on the recessed ground, so it reads as a state and not a row. */
const Empty: FC<{ title: string; children: ReactNode }> = (p) => (
	<div className={styles.empty}>
		<span className={classes("text-15", "text-semibold", styles.emptyTitle)}>{p.title}</span>
		<span className={classes("text-12", "text-body", styles.emptyHint)}>{p.children}</span>
	</div>
);

/** The project's linked git worktrees; archived ones are hidden from the workspace. */
export const Worktrees: FC<{ projectId: string }> = ({ projectId }) => {
	const { data: appSettings } = useSuspenseQuery(appSettingsQueryOptions);
	if (!appSettings.featureFlags.worktreeManipulation) {
		return (
			<Empty title="Linked worktrees are off">
				Turn them on under Experimental to list them here and show their commits in the workspace.
			</Empty>
		);
	}
	return <WorktreeList projectId={projectId} />;
};

const worktreeLabel = (worktree: ListedWorktree) =>
	worktree.refName === null
		? `${worktree.name} (detached)`
		: branchDetailsParams(worktree.refName).branchName;

const WorktreeList: FC<{ projectId: string }> = ({ projectId }) => {
	const { data: listing } = useSuspenseQuery(worktreesListQueryOptions(projectId));
	const { mutate: setArchived, isPending: isArchiving } = useWorktreeSetArchived(projectId);
	const { mutate: remove, isPending: isRemoving } = useWorktreeRemove(projectId);
	const busy = isArchiving || isRemoving;

	const rows = (worktrees: Array<ListedWorktree>, archived: boolean) =>
		worktrees.map((worktree) => (
			<div key={worktree.name} className={styles.row}>
				<div className={styles.text}>
					<span className={classes("text-15", "text-semibold", styles.name)}>
						<Icon name="folder" className={styles.folder} />
						{worktreeLabel(worktree)}
					</span>
					<span className={classes("text-12", "text-body", styles.path)} title={worktree.path}>
						<bdi dir="ltr">{worktree.path}</bdi>
					</span>
				</div>
				<div className={styles.actions}>
					<Button
						disabled={busy}
						onClick={() => setArchived({ projectId, name: worktree.name, archived: !archived })}
					>
						{archived ? "Unarchive" : "Archive"}
					</Button>
					<IconButton
						label="Worktree menu"
						onClick={(event) =>
							void showNativeMenuFromTrigger(event.currentTarget, [
								nativeMenuItem({
									label: revealInFolderLabel,
									onSelect: () => void window.lite.showItemInFolder(worktree.path),
								}),
								nativeMenuItem({
									label: "Copy Worktree Path",
									onSelect: () => void window.lite.clipboardWriteText(worktree.path),
								}),
								nativeMenuSeparator,
								nativeMenuItem({
									label: "Remove Worktree",
									enabled: !busy,
									onSelect: () => remove({ projectId, name: worktree.name, force: false }),
								}),
							])
						}
					>
						<Icon name="kebab" />
					</IconButton>
				</div>
			</div>
		));

	return (
		<>
			{listing.active.length === 0 ? (
				<Empty title="No active worktrees">
					A worktree added from now on shows up here and in the workspace.
				</Empty>
			) : (
				<Section heading="Active worktrees">{rows(listing.active, false)}</Section>
			)}
			{listing.archived.length > 0 && (
				<Section heading="Archived">{rows(listing.archived, true)}</Section>
			)}
		</>
	);
};
