import { useEnterEditMode } from "#ui/api/mutations.ts";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { Dialog } from "@base-ui/react";
import { useQuery } from "@tanstack/react-query";
import type { FC } from "react";
import styles from "./ConflictBar.module.css";
import { ConflictedFiles } from "#ui/routes/project/$id/workspace/ConflictedFiles.tsx";
import type {
	ConflictedFile,
	HunkResolution,
	ManualConflict,
	ResolutionSpec,
} from "@gitbutler/but-sdk";
import type { CheckedConflict } from "#ui/projects/project.ts";

type Props = {
	projectId: string;
	/** The conflicted commit the checks are scoped to. */
	commitId: string;
	/** Conflicted files that decompose into hunks. */
	conflicts: Array<ConflictedFile>;
	/** Conflicted files that can only be resolved in edit mode. */
	manual: Array<ManualConflict>;
	/** True while a resolution is in flight. */
	busy: boolean;
	onResolve: (specs: Array<ResolutionSpec>) => void;
};

/**
 * Flags what is unresolved in the selected commit and opens the resolution
 * dialog. The diff itself stays the commit's actual changes — at every
 * conflict the commit fell back to the parent, so there is nothing of it to
 * render there; resolving toward the authored side is what makes a change
 * appear in the diff.
 *
 * Also names the files that need edit mode, which have no marker view and
 * would otherwise only show in the closed-by-default files panel.
 */
export const ConflictBar: FC<Props> = (p) => {
	const dispatch = useAppDispatch();
	const checked = useAppSelector((state) =>
		projectSlice.selectors.selectCheckedConflicts(state, p.projectId, p.commitId),
	);

	// Edit mode wants the commit's stack; the bar finds it itself rather than
	// threading it through the details pane. Derived in `select` so the walk
	// runs when the head info changes rather than on every render, and the bar
	// subscribes to the id alone.
	const { data: stackId = null } = useQuery({
		...headInfoQueryOptions(p.projectId),
		select: (headInfo) =>
			headInfo.stacks.find((stack) =>
				stack.segments.some((segment) =>
					segment.commits.some((commit) => commit.id === p.commitId),
				),
			)?.id ?? null,
	});
	const { mutate: enterEditMode } = useEnterEditMode(p.projectId);

	const total = p.conflicts.reduce((sum, file) => sum + file.hunks.length, 0);
	if (total === 0 && p.manual.length === 0) return null;

	// Ids resolve to current positions; stale checks match nothing.
	const liveByPath = new Map(p.conflicts.map((file) => [file.path, file]));
	const checkedLive = checked
		.values()
		.map(
			(check) =>
				[
					check,
					liveByPath.get(check.path)?.hunks.findIndex((hunk) => hunk.id === check.id),
				] as const,
		)
		.filter((pair): pair is [CheckedConflict, number] => pair[1] != null && pair[1] !== -1)
		.map(([check, index]) => ({ path: check.path, hunk: index + 1 }))
		.toArray();

	const apply = (resolution: HunkResolution) => {
		if (p.busy || checkedLive.length === 0) return;
		p.onResolve(checkedLive.map(({ path, hunk }) => ({ path, hunk, resolution })));
	};

	return (
		<div className={styles.bar}>
			<Icon name="warning" />
			<span className={classes(styles.status, "text-13")}>
				{`${total} conflict${total === 1 ? "" : "s"} auto-resolved — the change${
					total === 1 ? "" : "s"
				} as authored could not be applied`}
			</span>

			{p.manual.length > 0 && (
				<span
					className={classes(styles.manual, "text-12")}
					title={p.manual.map((file) => `${file.path} — ${file.reason}`).join("\n")}
				>
					{p.manual.length} file{p.manual.length === 1 ? "" : "s"} can only be resolved in edit mode
				</span>
			)}

			<button
				type="button"
				className={classes(getButtonClassName({ variant: "outline", size: "small" }))}
				disabled={p.busy || stackId === null}
				title="Check the commit out into your working directory and edit its files directly"
				onClick={() => {
					if (stackId !== null)
						enterEditMode({ projectId: p.projectId, commitId: p.commitId, stackId });
				}}
			>
				Open Edit Mode
			</button>

			{total > 0 && (
				<Dialog.Root>
					<Dialog.Trigger
						className={classes(
							getButtonClassName({ variant: "outline", size: "small" }),
							styles.resolve,
						)}
					>
						Resolve conflicts
					</Dialog.Trigger>
					<Dialog.Portal>
						<Dialog.Backdrop className={styles.backdrop} />
						<Dialog.Viewport className={styles.viewport}>
							<Dialog.Popup aria-labelledby="resolve-conflicts-heading" className={styles.popup}>
								<header className={styles.header}>
									<Icon name="warning" />
									<h1
										id="resolve-conflicts-heading"
										className={classes("text-14", "text-bold", styles.heading)}
									>
										{checkedLive.length > 0
											? `${checkedLive.length} of ${total} conflict${total === 1 ? "" : "s"} selected`
											: `Resolve ${total} conflict${total === 1 ? "" : "s"}`}
									</h1>

									{checkedLive.length > 0 && (
										<div className={styles.actions}>
											<button
												type="button"
												className={getButtonClassName({ variant: "outline", size: "small" })}
												disabled={p.busy}
												onClick={() => apply({ type: "theirs" })}
											>
												Accept incoming
											</button>
											<button
												type="button"
												className={getButtonClassName({ variant: "outline", size: "small" })}
												disabled={p.busy}
												onClick={() => apply({ type: "ours" })}
											>
												Accept current
											</button>
											<button
												type="button"
												className={getButtonClassName({ variant: "ghost", size: "small" })}
												onClick={() =>
													dispatch(
														projectSlice.actions.clearCheckedConflicts({ projectId: p.projectId }),
													)
												}
											>
												Clear
											</button>
										</div>
									)}

									<Dialog.Close
										aria-label="Close"
										className={getButtonClassName({
											variant: "ghost",
											size: "small",
											iconOnly: true,
										})}
									>
										<Icon name="cross" />
									</Dialog.Close>
								</header>

								<div className={styles.body}>
									<ConflictedFiles
										projectId={p.projectId}
										commitId={p.commitId}
										conflicts={p.conflicts}
										busy={p.busy}
										onResolve={p.onResolve}
									/>
								</div>
							</Dialog.Popup>
						</Dialog.Viewport>
					</Dialog.Portal>
				</Dialog.Root>
			)}
		</div>
	);
};
