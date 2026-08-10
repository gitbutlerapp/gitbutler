import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import type { FC } from "react";
import styles from "./ConflictBar.module.css";
import type {
	ConflictedFile,
	HunkResolution,
	ManualConflict,
	ResolutionSpec,
} from "@gitbutler/but-sdk";

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
	/** Whether the diff below is showing two columns; the sides are named for
	 * where they appear, and only a split has a left and right. */
	splitView: boolean;
	onResolve: (specs: Array<ResolutionSpec>) => void;
};

/**
 * What is still unresolved in the selected commit, and the actions for the
 * conflicts that are checked.
 *
 * Present for the whole time a commit is conflicted rather than appearing with
 * the first check, so working down the list doesn't shift the diff under the
 * pointer. It also carries the files that need edit mode: they have no diff and
 * no cards, so the only other place they appear is the files panel, which is
 * closed by default.
 */
export const ConflictBar: FC<Props> = (p) => {
	const dispatch = useAppDispatch();
	const checked = useAppSelector((state) =>
		projectSlice.selectors.selectCheckedConflicts(state, p.projectId, p.commitId),
	);

	const total = p.conflicts.reduce((sum, file) => sum + file.hunks.length, 0);
	if (total === 0 && p.manual.length === 0) return null;

	const apply = (resolution: HunkResolution) => {
		if (p.busy || checked.length === 0) return;
		p.onResolve(checked.map(({ path, hunk }) => ({ path, hunk, resolution })));
	};

	return (
		<div className={styles.bar}>
			<Icon name="warning" />
			<span className={classes(styles.status, "text-13")}>
				{checked.length > 0
					? `${checked.length} of ${total} conflict${total === 1 ? "" : "s"} selected`
					: `${total} unresolved conflict${total === 1 ? "" : "s"}`}
			</span>

			{p.manual.length > 0 && (
				<span
					className={classes(styles.manual, "text-12")}
					title={p.manual.map((file) => `${file.path} — ${file.reason}`).join("\n")}
				>
					{p.manual.length} file{p.manual.length === 1 ? "" : "s"} can only be resolved in edit mode
				</span>
			)}

			{checked.length > 0 && (
				<div className={styles.actions}>
					<button
						type="button"
						className={getButtonClassName({ variant: "outline", size: "small" })}
						disabled={p.busy}
						onClick={() => apply({ type: "ours" })}
					>
						{p.splitView ? "Use left" : "Use removed"}
					</button>
					<button
						type="button"
						className={getButtonClassName({ variant: "outline", size: "small" })}
						disabled={p.busy}
						onClick={() => apply({ type: "theirs" })}
					>
						{p.splitView ? "Use right" : "Use added"}
					</button>
					<button
						type="button"
						className={getButtonClassName({ variant: "ghost", size: "small" })}
						onClick={() =>
							dispatch(projectSlice.actions.clearCheckedConflicts({ projectId: p.projectId }))
						}
					>
						Clear
					</button>
				</div>
			)}
		</div>
	);
};
