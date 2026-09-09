import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { getRowButtonClassName } from "#ui/routes/project/$id/workspace/Row-utils.ts";
import { Button, Popover, Tooltip } from "@base-ui/react";
import type { FC } from "react";
import { Dropdown } from "#ui/components/Popup.tsx";
import { FileIcon } from "#ui/components/FileIcon.tsx";
import { classes } from "#ui/components/classes.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import styles from "./Integrate.module.css";
import type { useWorkspaceIntegrationPreview } from "./useWorkspaceIntegrationPreview.ts";

export const IntegrationStatus: FC<{
	target: string;
	preview: ReturnType<typeof useWorkspaceIntegrationPreview>;
}> = ({ target, preview }) => {
	const { conflicts } = preview;
	if (
		conflicts === undefined ||
		(conflicts.files.length === 0 && conflicts.branches.length === 0 && !conflicts.checkoutConflict)
	)
		return null;
	return (
		<Dropdown
			className={styles.popup}
			aria-label="Base update conflicts"
			trigger={
				<button
					type="button"
					className={classes(
						getRowButtonClassName({ variant: "ghost", iconOnly: true }),
						styles.warning,
					)}
					aria-label="Update will cause conflicts"
				>
					<Icon name="warning" />
				</button>
			}
		>
			<div className={styles.header}>
				<div>
					<h3 className={styles.heading}>Update conflicts</h3>
					<p className={styles.description}>
						From <span className={styles.target}>{target}</span>
					</p>
				</div>
				<Popover.Close
					type="button"
					aria-label="Close conflict details"
					className={classes(
						getButtonClassName({ variant: "ghost", size: "small", iconOnly: true }),
						styles.close,
					)}
				>
					<Icon name="cross" />
				</Popover.Close>
			</div>
			<div className={styles.details}>
				{conflicts.branches.map((branch) => (
					<section key={branch.name} className={styles.group}>
						<h4 className={styles.branch}>
							<Icon name="branch" />
							<span className={styles.branchName}>{branch.name}</span>
							<span className={styles.count}>{branch.commits.length}</span>
						</h4>
						<ul className={styles.list}>
							{branch.commits.map((commit) => (
								<li key={commit.id}>
									<div className={styles.commit}>
										<Icon name="commit" />
										<span className={styles.commitTitle}>{commit.title}</span>
										<code>{commit.id.slice(0, 7)}</code>
									</div>
									<ul
										className={classes(styles.list, styles.commitFiles)}
										aria-label="Conflicted files"
									>
										{commit.files.map((file) => (
											<li className={styles.file} key={file}>
												<FileIcon fileName={file.slice(file.lastIndexOf("/") + 1)} />
												<span>{file}</span>
											</li>
										))}
									</ul>
								</li>
							))}
						</ul>
					</section>
				))}
				{conflicts.files.length > 0 && (
					<section className={styles.group}>
						<h4 className={styles.branch}>
							<Icon name="file" />
							<span className={styles.branchName}>Uncommitted files</span>
							<span className={styles.count}>{conflicts.files.length}</span>
						</h4>
						<p className={styles.fileDescription}>These files will conflict after the update</p>
						<ul className={styles.list}>
							{conflicts.files.map((file) => (
								<li className={styles.file} key={file}>
									<FileIcon fileName={file.slice(file.lastIndexOf("/") + 1)} />
									<span>{file}</span>
								</li>
							))}
						</ul>
					</section>
				)}
				{conflicts.checkoutConflict && (
					<p className={styles.description}>
						The updated branches also conflict when combined in the workspace
					</p>
				)}
			</div>
		</Dropdown>
	);
};

/** Rebases every stack onto the target's fetched tip; this does not fetch. */
export const Integrate: FC<{
	target: string;
	preview: ReturnType<typeof useWorkspaceIntegrationPreview>;
}> = ({ target, preview }) => {
	const { enabled, isPending, rebase } = preview;
	return (
		<Tooltip.Root>
			<Tooltip.Trigger
				className={getRowButtonClassName({ variant: "outline" })}
				onClick={rebase}
				// `disabled` goes on the button so the tooltip still opens over it.
				render={<Button focusableWhenDisabled disabled={!enabled} />}
			>
				{isPending ? "Pulling…" : "Pull latest"}
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>
						Pull the latest from {target} into the workspace base
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
