import { useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FC, type ReactNode, useEffect, useState } from "react";
import {
	editChangesFromInitialQueryOptions,
	editInitialIndexStateQueryOptions,
	guiSettingsQueryOptions,
	listEditorsQueryOptions,
	workspaceFileQueryOptions,
} from "#ui/api/queries.ts";
import {
	useAbortEditAndReturnToWorkspace,
	useOpenInProgram,
	useSaveEditAndReturnToWorkspace,
} from "#ui/api/mutations.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { ConflictIcon } from "#ui/components/ConflictIcon.tsx";
import { FileIcon } from "#ui/components/FileIcon.tsx";
import { nativeMenuItem, showNativeContextMenu } from "#ui/native-menu.ts";
import type { ConflictEntryPresence, EditModeMetadata } from "@gitbutler/but-sdk";
import { type ConflictState, conflictHint, conflictStateOf } from "./edit-mode-conflicts.ts";
import { usePathMenuItems } from "./usePathMenuItems.ts";
import styles from "./EditModePage.module.css";

const fileName = (path: string): string => path.slice(path.lastIndexOf("/") + 1);

/**
 * The cache-busting version for reads that follow the working tree. It is a
 * constant so each file keeps one entry, which invalidation refreshes in place.
 */
const LIVE = 0;

/**
 * One file in edit mode. It owns its menu so the menu is built for the row
 * the user actually opened, not for every row on every render.
 */
const EditModeFileRow: FC<{
	projectId: string;
	path: string;
	icon: ReactNode;
	hint?: ReactNode;
	onMarkResolved?: () => void;
}> = ({ projectId, path, icon, hint, onMarkResolved }) => {
	const pathMenuItems = usePathMenuItems({ projectId, path });
	const menuItems = [
		...pathMenuItems,
		...(onMarkResolved
			? [nativeMenuItem({ label: "Mark as Resolved", onSelect: onMarkResolved })]
			: []),
	];

	return (
		<div
			className={styles.fileRow}
			onContextMenu={(event) => void showNativeContextMenu(event, menuItems)}
		>
			{icon}
			<span className={styles.filePath}>{path}</span>
			{hint}
		</div>
	);
};

/**
 * The workspace surface while the repository is in edit mode: the edited
 * commit's files are checked out for real, so any editor or agent can work
 * on them. Save rewrites the commit and rebases what sat above it; cancel
 * puts the workspace back untouched.
 */
export const EditModePage: FC<{ projectId: string; metadata: EditModeMetadata }> = ({
	projectId,
	metadata,
}) => {
	const queryClient = useQueryClient();
	const { data: initialFiles } = useQuery(editInitialIndexStateQueryOptions(projectId));
	const { data: changedFiles, dataUpdatedAt } = useQuery(
		editChangesFromInitialQueryOptions(projectId),
	);

	const { mutate: saveEdit, isPending: saving } = useSaveEditAndReturnToWorkspace(projectId);
	const { mutate: abortEdit, isPending: aborting } = useAbortEditAndReturnToWorkspace(projectId);
	const { mutate: openInProgram } = useOpenInProgram();
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: preferredEditor } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => editors?.find((editor) => editor.id === cfg.editorId),
	});
	const busy = saving || aborting;

	// Files the user has declared done despite what their text still says.
	const [manuallyResolved, setManuallyResolved] = useState<ReadonlySet<string>>(new Set());

	const conflicted = (initialFiles ?? []).filter(
		(entry): entry is [(typeof entry)[0], ConflictEntryPresence] => entry[1] != null,
	);

	// One cache entry per file, refreshed when the watcher reports the worktree
	// moved. Keying on the tick instead would read every conflicted file again
	// on every tick and strand the old entry, each holding a whole file.
	const conflictedPaths = conflicted.map(([change]) => change.path).join("\n");
	useEffect(() => {
		for (const path of conflictedPaths === "" ? [] : conflictedPaths.split("\n")) {
			void queryClient.invalidateQueries({
				queryKey: workspaceFileQueryOptions({
					projectId,
					relativePath: path,
					version: LIVE,
				}).queryKey,
			});
		}
	}, [conflictedPaths, dataUpdatedAt, projectId, queryClient]);

	const conflictStates = useQueries({
		queries: conflicted.map(([change]) => ({
			...workspaceFileQueryOptions({
				projectId,
				relativePath: change.path,
				version: LIVE,
			}),
		})),
		combine: (results) =>
			new Map<string, ConflictState>(
				conflicted.map(([change, presence], index) => [
					change.path,
					conflictStateOf(presence, results[index]?.data),
				]),
			),
	});

	const stateOf = (path: string): ConflictState =>
		manuallyResolved.has(path) ? "resolved" : (conflictStates.get(path) ?? "unknown");

	const markResolved = (path: string) => setManuallyResolved((paths) => new Set(paths).add(path));

	const openConflictedFiles = () => {
		if (!preferredEditor) return;

		for (const [change] of conflicted)
			openInProgram({ projectId, programId: preferredEditor.id, path: change.path, lineNr: null });
	};

	/**
	 * Asked at save time rather than read off the rendered state, which can lag
	 * the disk: the answer decides whether we warn about conflicts. A file we
	 * cannot read counts as unresolved — the rows say the same, and marking it
	 * resolved by hand is how the user overrules that.
	 */
	const unresolvedOnDisk = async (): Promise<number> => {
		const states = await Promise.all(
			conflicted.map(async ([change, presence]) => {
				if (manuallyResolved.has(change.path)) return "resolved" as const;
				const file = await queryClient.fetchQuery({
					...workspaceFileQueryOptions({
						projectId,
						relativePath: change.path,
						version: LIVE,
					}),
					// The client defaults staleTime to Infinity, so without this the
					// fetch answers from cache and never sees the current file.
					staleTime: 0,
				});
				return conflictStateOf(presence, file);
			}),
		);
		return states.filter((state) => state !== "resolved").length;
	};

	const save = async () => {
		const unresolved = await unresolvedOnDisk();
		if (
			unresolved > 0 &&
			!window.confirm(
				`${unresolved} conflicted ${
					unresolved === 1 ? "file is" : "files are"
				} not resolved. Save the commit with the conflicts still in?`,
			)
		)
			return;
		saveEdit(projectId);
	};

	const cancel = async () => {
		// Asked at cancel time for the same reason saving asks: the rendered list
		// trails the disk, and abandoning unacknowledged edits is not undoable.
		const changed = await queryClient.fetchQuery({
			...editChangesFromInitialQueryOptions(projectId),
			staleTime: 0,
		});
		if (changed.length > 0 && !window.confirm("Discard the changes made in edit mode?")) return;
		abortEdit({ projectId, force: true });
	};

	return (
		<div className={styles.page}>
			<div className={styles.panel}>
				<h1 className={styles.title}>Editing commit</h1>
				<span className={styles.commitRef}>{metadata.commitOid.slice(0, 10)}</span>
				<p className={styles.explainer}>
					This commit is checked out in your working directory. Edit its files with any tool —
					resolve conflicts, tweak the change — then save to rewrite the commit. Everything that was
					stacked on top gets rebased onto the result.
				</p>

				<h2 className={styles.sectionTitle}>Files in this commit</h2>
				<div className={styles.files}>
					{(initialFiles ?? []).map(([change, presence]) => (
						<EditModeFileRow
							key={change.path}
							projectId={projectId}
							path={change.path}
							icon={
								presence != null ? (
									<ConflictIcon variant="conflict" />
								) : (
									<FileIcon fileName={fileName(change.path)} />
								)
							}
							hint={
								presence != null &&
								(stateOf(change.path) === "resolved" ? (
									<span className={styles.resolvedHint}>resolved</span>
								) : (
									<span className={styles.conflictHint}>{conflictHint(presence)}</span>
								))
							}
							onMarkResolved={
								presence != null && stateOf(change.path) !== "resolved"
									? () => markResolved(change.path)
									: undefined
							}
						/>
					))}
					{initialFiles?.length === 0 && <div className={styles.empty}>An empty commit.</div>}
				</div>

				<h2 className={styles.sectionTitle}>Changed since entering edit mode</h2>
				<div className={styles.files}>
					{(changedFiles ?? []).map((change) => (
						<EditModeFileRow
							key={change.path}
							projectId={projectId}
							path={change.path}
							icon={<FileIcon fileName={fileName(change.path)} />}
						/>
					))}
					{changedFiles?.length === 0 && <div className={styles.empty}>No changes yet.</div>}
				</div>

				<div className={styles.buttons}>
					{conflicted.length > 0 && preferredEditor && (
						<button
							type="button"
							className={classes(getButtonClassName({ variant: "outline" }), styles.openAll)}
							onClick={openConflictedFiles}
						>
							{conflicted.length === 1
								? `Open conflicted file in ${preferredEditor.name}`
								: `Open ${conflicted.length} conflicted files in ${preferredEditor.name}`}
						</button>
					)}
					<button
						type="button"
						className={getButtonClassName({ variant: "pop" })}
						disabled={busy}
						onClick={() => void save()}
					>
						Save and return
					</button>
					<button
						type="button"
						className={classes(getButtonClassName({ variant: "outline" }))}
						disabled={busy}
						onClick={() => void cancel()}
					>
						Cancel edit
					</button>
				</div>
			</div>
		</div>
	);
};
