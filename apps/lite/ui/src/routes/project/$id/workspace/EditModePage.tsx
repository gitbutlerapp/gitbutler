import { useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FC, type ReactNode, useState } from "react";
import {
	editChangesFromInitialQueryOptions,
	editInitialIndexStateQueryOptions,
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
import { guiSettingsQueryOptions, listEditorsQueryOptions } from "#ui/api/queries.ts";
import type { ConflictEntryPresence, EditModeMetadata } from "@gitbutler/but-sdk";
import { type ConflictState, conflictHint, conflictStateOf } from "./edit-mode-conflicts.ts";
import { usePathMenuItems } from "./usePathMenuItems.ts";
import styles from "./EditModePage.module.css";

const fileName = (path: string): string => path.slice(path.lastIndexOf("/") + 1);

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

	const { mutate: saveEdit, isPending: saving } = useSaveEditAndReturnToWorkspace();
	const { mutate: abortEdit, isPending: aborting } = useAbortEditAndReturnToWorkspace();
	const { mutate: openInProgram } = useOpenInProgram();
	const { data: editors } = useQuery(listEditorsQueryOptions);
	const { data: preferredEditor } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => editors?.find((editor) => editor.id === cfg.editorId),
	});
	const busy = saving || aborting;

	// Files the user has declared done despite what their text still says.
	const [manuallyResolved, setManuallyResolved] = useState<ReadonlySet<string>>(new Set());

	const changedPaths = new Set(changedFiles?.map((change) => change.path));
	const conflicted = (initialFiles ?? []).filter(
		(entry): entry is [(typeof entry)[0], ConflictEntryPresence] => entry[1] != null,
	);

	// Re-read on every refetch of the changed-files query, which the watcher
	// drives: that is when the text on disk can have moved under us.
	const conflictStates = useQueries({
		queries: conflicted.map(([change]) => ({
			...workspaceFileQueryOptions({
				projectId,
				relativePath: change.path,
				version: dataUpdatedAt,
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

	// A button cannot offer a choice the way the file menu does, so with no
	// preferred editor set it falls back to the first one rather than leaving
	// the conflicts with no way to open them at all.
	const openAllEditor = preferredEditor ?? editors?.[0];

	const openConflictedFiles = () => {
		if (!openAllEditor) return;
		for (const [change] of conflicted)
			openInProgram({ projectId, programId: openAllEditor.id, path: change.path, lineNr: null });
	};

	/**
	 * Asked at save time rather than read off the rendered state, which can
	 * lag the disk: the answer decides whether we warn about conflicts.
	 */
	const unresolvedOnDisk = async (): Promise<number> => {
		const states = await Promise.all(
			conflicted.map(async ([change, presence]) => {
				if (manuallyResolved.has(change.path)) return "resolved" as const;
				const file = await queryClient.fetchQuery({
					...workspaceFileQueryOptions({
						projectId,
						relativePath: change.path,
						version: dataUpdatedAt,
					}),
					// The client defaults staleTime to Infinity, so without this the
					// fetch answers from cache and never sees the current file.
					staleTime: 0,
				});
				return conflictStateOf(presence, file);
			}),
		);
		return states.filter((state) => state === "conflicted").length;
	};

	const save = async () => {
		const unresolved = await unresolvedOnDisk();
		if (
			unresolved > 0 &&
			!window.confirm(
				`${unresolved} conflicted ${
					unresolved === 1 ? "file still has" : "files still have"
				} conflict markers. Save the commit with its conflicts still in?`,
			)
		)
			return;
		saveEdit(projectId);
	};

	const cancel = () => {
		if (changedPaths.size > 0 && !window.confirm("Discard the changes made in edit mode?")) return;
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
					{conflicted.length > 0 && openAllEditor && (
						<button
							type="button"
							className={classes(getButtonClassName({ variant: "outline" }), styles.openAll)}
							onClick={openConflictedFiles}
						>
							{conflicted.length === 1
								? `Open conflicted file in ${openAllEditor.name}`
								: `Open ${conflicted.length} conflicted files in ${openAllEditor.name}`}
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
						onClick={cancel}
					>
						Cancel edit
					</button>
				</div>
			</div>
		</div>
	);
};
