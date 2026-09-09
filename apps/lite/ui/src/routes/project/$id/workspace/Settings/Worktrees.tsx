import { useSuspenseQuery } from "@tanstack/react-query";
import type { FC } from "react";
import { appSettingsQueryOptions, worktreesListQueryOptions } from "#ui/api/queries.ts";
import { useWorktreeRemove, useWorktreeSetArchived } from "#ui/api/mutations.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { branchDetailsParams } from "#ui/branch.ts";
import type { ListedWorktree } from "@gitbutler/but-sdk";
import styles from "./Worktrees.module.css";
import { Row, Section } from "./Section.tsx";

/** The project's linked git worktrees; archived ones are hidden from the workspace. */
export const Worktrees: FC<{ projectId: string }> = ({ projectId }) => {
	const { data: appSettings } = useSuspenseQuery(appSettingsQueryOptions);
	if (!appSettings.featureFlags.worktreeManipulation) {
		return (
			<Section>
				<Row
					label="Linked worktrees are off"
					hint="Turn them on under Experimental to list them here and show their commits in the workspace."
				/>
			</Section>
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
			<Row
				key={worktree.name}
				label={worktreeLabel(worktree)}
				hint={
					<span className={styles.path} title={worktree.path}>
						<bdi dir="ltr">{worktree.path}</bdi>
					</span>
				}
			>
				<div className={styles.actions}>
					<button
						type="button"
						className={getButtonClassName({ size: "small" })}
						disabled={busy}
						onClick={() => setArchived({ projectId, name: worktree.name, archived: !archived })}
					>
						{archived ? "Unarchive" : "Archive"}
					</button>
					<button
						type="button"
						className={getButtonClassName({ variant: "danger", size: "small" })}
						disabled={busy}
						onClick={() => remove({ projectId, name: worktree.name, force: false })}
					>
						Remove
					</button>
				</div>
			</Row>
		));

	return (
		<>
			<Section>
				{listing.active.length === 0 ? (
					<Row
						label="No active worktrees"
						hint="A worktree added from now on shows up here and in the workspace."
					/>
				) : (
					rows(listing.active, false)
				)}
			</Section>
			{listing.archived.length > 0 && (
				<Section heading="Archived">{rows(listing.archived, true)}</Section>
			)}
		</>
	);
};
