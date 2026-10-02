import { useQuery, useSuspenseQuery } from "@tanstack/react-query";
import { type FC, type ReactNode, useState } from "react";
import {
	appSettingsQueryOptions,
	hostedBranchesQueryOptions,
	worktreesListQueryOptions,
} from "#ui/api/queries.ts";
import {
	useHostedBranchPull,
	useWorktreeRemove,
	useWorktreeSetArchived,
} from "#ui/api/mutations.ts";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { Modal, ModalFooter, ModalHeader } from "@gitbutler/ui-react/Popup.tsx";
import { EmptyState } from "@gitbutler/ui-react/EmptyState.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { branchDetailsParams } from "#ui/branch.ts";
import { revealInFolderLabel } from "#ui/hotkeys.ts";
import { nativeMenuItem, nativeMenuSeparator, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import type { HostedBranch, ListedWorktree } from "@gitbutler/but-sdk";
import { IconButton } from "./IconButton.tsx";
import styles from "./Worktrees.module.css";
import { Section } from "./Section.tsx";

/** A section with nothing to list: the library's empty state, framed on the recessed ground so it
 * reads as the state of the page and not as one more row of it. */
const Empty: FC<{ title: string; children: ReactNode }> = (p) => (
	<div className={styles.empty}>
		<EmptyState title={p.title} description={p.children} />
	</div>
);

/** The project's linked git worktrees; archived ones are hidden from the workspace. */
export const Worktrees: FC<{ projectId: string }> = ({ projectId }) => {
	const { data: appSettings } = useSuspenseQuery(appSettingsQueryOptions);
	if (!appSettings.featureFlags.worktreeManipulation) {
		return (
			<Empty title="Linked worktrees are off">
				Turn them on under Experimental to see them here and in the workspace.
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
			{window.lite.hosted !== true && <Published projectId={projectId} />}
		</>
	);
};

const whereLocal = (local: HostedBranch["local"]) => {
	switch (local.type) {
		case "worktree":
			return ", local in a worktree";
		case "workspace":
			return ", local in the workspace";
		case "branch":
			return ", local branch";
		case "none":
			return "";
	}
};

/** Branches published to the hosted server from anywhere, to pull down here. */
const Published: FC<{ projectId: string }> = ({ projectId }) => {
	// A server that can't be reached just has nothing to list.
	const { data: published } = useQuery(hostedBranchesQueryOptions(projectId));
	const { mutate, isPending } = useHostedBranchPull(projectId);
	// A pull that would replace local work, waiting for the answer.
	const [asking, setAsking] = useState<{
		branch: string;
		intoWorkspace: boolean;
		reason: string;
	}>();
	if (published === undefined || published.length === 0) return null;

	const pull = (branch: string, intoWorkspace: boolean, overwrite: boolean) =>
		mutate(
			{ projectId, branch, intoWorkspace, onConflict: overwrite ? "overwrite" : null },
			{
				onSuccess: (outcome) => {
					if (outcome.type === "needsChoice")
						setAsking({ branch, intoWorkspace, reason: outcome.subject });
				},
			},
		);

	return (
		<Section heading="Published">
			{published.map(({ branch, uncommitted, local }) => (
				<div key={branch} className={styles.row}>
					<div className={styles.text}>
						<span className={classes("text-15", "text-semibold", styles.name)}>
							<Icon name="globe" className={styles.folder} />
							{branch}
						</span>
						<span className={classes("text-12", "text-body", styles.path)}>
							{uncommitted ? "With uncommitted changes" : "Committed changes only"}
							{whereLocal(local)}
						</span>
					</div>
					<div className={styles.actions}>
						{local.type === "worktree" || local.type === "workspace" ? (
							<Button disabled={isPending} onClick={() => pull(branch, false, false)}>
								Update
							</Button>
						) : (
							<>
								<Button disabled={isPending} onClick={() => pull(branch, true, false)}>
									Pull into workspace
								</Button>
								<Button disabled={isPending} onClick={() => pull(branch, false, false)}>
									Pull
								</Button>
							</>
						)}
					</div>
				</div>
			))}
			<Modal
				open={asking !== undefined}
				onOpenChange={(open) => {
					if (!open) setAsking(undefined);
				}}
			>
				<ModalHeader title="Overwrite local work?" description={asking?.reason} />
				<ModalFooter>
					<Button variant="ghost" onClick={() => setAsking(undefined)}>
						Keep local
					</Button>
					<Button
						variant="gray"
						onClick={() => {
							if (asking) pull(asking.branch, asking.intoWorkspace, true);
							setAsking(undefined);
						}}
					>
						Overwrite
					</Button>
				</ModalFooter>
			</Modal>
		</Section>
	);
};
