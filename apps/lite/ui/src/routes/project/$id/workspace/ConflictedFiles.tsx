import { guiSettingsQueryOptions } from "#ui/api/queries.ts";
import { Badge } from "#ui/components/Badge.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Checkbox } from "#ui/components/Checkbox.tsx";
import { classes } from "#ui/components/classes.ts";
import { FileIcon } from "#ui/components/FileIcon.tsx";
import { projectSlice } from "#ui/projects/state.ts";
import { defaultSettings } from "#ui/settings.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import {
	DEFAULT_THEMES,
	getFiletypeFromFileName,
	getSharedHighlighter,
	getThemes,
	type SupportedLanguages,
} from "@pierre/diffs";
import { Editor, type EditorOptions } from "@pierre/diffs/edit";
import { EditProvider, File, UnresolvedFile } from "@pierre/diffs/react";
import { useQuery } from "@tanstack/react-query";
import { type ComponentProps, type FC, useEffect, useRef, useState } from "react";
import styles from "./ConflictedFiles.module.css";
import type {
	ConflictedFile,
	ConflictHunk,
	HunkResolution,
	ResolutionSpec,
} from "@gitbutler/but-sdk";

type UnresolvedFileProps = ComponentProps<typeof UnresolvedFile>;
type FileProps = ComponentProps<typeof File>;

/** Components own the instance lifecycle; this only constructs. */
const createEditor = (options: EditorOptions<unknown>) => new Editor(options);

type Props = {
	projectId: string;
	/** The conflicted commit, which resolutions and checks address. */
	commitId: string;
	conflicts: Array<ConflictedFile>;
	/**
	 * True while any resolution is in flight. Each apply rewrites the commit,
	 * so a second one started meanwhile would address a dead id.
	 */
	busy: boolean;
	onResolve: (specs: Array<ResolutionSpec>) => void;
};

/**
 * The selected commit's conflicted files, rendered from their marker text by
 * the diff library's merge-conflict view: each conflict diffs what the
 * auto-resolution kept against the change as authored, with our resolution
 * controls in the per-conflict slot.
 */
export const ConflictedFiles: FC<Props> = (p) => {
	// The library highlights through a shared highlighter something else must
	// load — normally a mounted CodeView, but a fully conflicted commit mounts
	// none, and a view rendered before the load stays plain text forever. Hold
	// rendering until the highlighter has these files' languages attached; the
	// ready set only grows, so a resolution that drops a language never blanks
	// the views that remain.
	const [readyLanguages, setReadyLanguages] = useState<ReadonlySet<string>>(new Set());
	const languages = [...new Set(p.conflicts.map((file) => getFiletypeFromFileName(file.path)))]
		.toSorted()
		// Plain text never loads anything; treating it as a language would hold
		// rendering forever.
		.filter((lang) => lang !== "text" && lang !== "ansi");
	const languagesKey = languages.join();
	useEffect(() => {
		let stale = false;
		void getSharedHighlighter({
			langs: languagesKey.split(",").filter(Boolean) as Array<SupportedLanguages>,
			themes: getThemes(DEFAULT_THEMES),
		}).then(() => {
			if (!stale)
				setReadyLanguages((previous) => new Set([...previous, ...languagesKey.split(",")]));
		});
		return () => {
			stale = true;
		};
	}, [languagesKey]);

	const { data: settings } = useQuery({
		...guiSettingsQueryOptions,
		select: (cfg) => ({
			diffFontFamily: cfg.diffFontFamily,
			diffFontSize: cfg.diffFontSize,
			diffLigatures: cfg.diffLigatures,
			diffTabSize: cfg.diffTabSize,
			lineDiffType: cfg.lineDiffType,
			theme: cfg.theme,
			diffBackground: cfg.diffBackground,
			diffOverflow: cfg.diffOverflow,
		}),
	});

	const options: UnresolvedFileProps["options"] = {
		// FileDiff derives this from renderCustomHeader; UnresolvedFile's option
		// merge forgets to, so without it the default header renders on top of ours.
		headerRenderMode: "custom",
		themeType: settings?.theme ?? defaultSettings.theme,
		overflow: settings?.diffOverflow ?? defaultSettings.diffOverflow,
		disableBackground: !(settings?.diffBackground ?? defaultSettings.diffBackground),
		lineDiffType: settings?.lineDiffType ?? defaultSettings.lineDiffType,
		unsafeCSS: `
      :host {
        background-color: transparent;
        /* Inherited, so this reaches the code inside the shadow root — which is the
           only way in, since ligatures are not one of Pierre's options. */
        font-variant-ligatures: ${
					(settings?.diffLigatures ?? defaultSettings.diffLigatures) ? "normal" : "none"
				};
      }

      [data-diffs-header="custom"] {
        background-color: var(--bg-1);
      }

      /* The slotted editor spans the actions row on its own line. */
      [data-merge-conflict-actions-content] {
        flex-wrap: wrap;
      }
    `,
	};

	// The editor the Edit action opens: a bare editable code surface — the
	// conflict view above it is the header and the line numbers would count
	// the region, not the file.
	const editFileOptions: FileProps["options"] = {
		themeType: settings?.theme ?? defaultSettings.theme,
		overflow: settings?.diffOverflow ?? defaultSettings.diffOverflow,
		disableFileHeader: true,
		disableLineNumbers: true,
	};

	const style: UnresolvedFileProps["style"] = {
		"--diffs-font-family": settings?.diffFontFamily ?? defaultSettings.diffFontFamily,
		"--diffs-font-size": `${settings?.diffFontSize ?? defaultSettings.diffFontSize}px`,
		"--diffs-tab-size": `${settings?.diffTabSize ?? defaultSettings.diffTabSize}`,
	};

	if (!languages.every((lang) => readyLanguages.has(lang))) return null;

	return (
		<EditProvider createEditor={createEditor}>
			{p.conflicts.map((file) => (
				<ConflictedFileC
					// Contents are read once per mount and change exactly when the hunk
					// set does. Never key on the commit id: it updates ahead of the refetch.
					key={`${file.path}:${file.hunks.map((hunk) => hunk.id).join()}`}
					projectId={p.projectId}
					commitId={p.commitId}
					file={file}
					busy={p.busy}
					onResolve={p.onResolve}
					options={options}
					editFileOptions={editFileOptions}
					style={style}
				/>
			))}
		</EditProvider>
	);
};

const ConflictedFileC: FC<{
	projectId: string;
	commitId: string;
	file: ConflictedFile;
	busy: boolean;
	onResolve: (specs: Array<ResolutionSpec>) => void;
	options: UnresolvedFileProps["options"];
	editFileOptions: FileProps["options"];
	style: UnresolvedFileProps["style"];
}> = (p) => {
	const lastSepIdx = p.file.path.lastIndexOf("/");
	const directoryPath = lastSepIdx !== -1 ? p.file.path.slice(0, lastSepIdx) : null;
	const fileName = lastSepIdx !== -1 ? p.file.path.slice(lastSepIdx + 1) : p.file.path;

	return (
		<section className={styles.file}>
			<UnresolvedFile
				file={{ name: p.file.path, contents: p.file.mergedText }}
				className={styles.diff}
				style={p.style}
				options={p.options}
				// The pool highlights with its own worker-side highlighter and this
				// view never re-renders when that boots, so a cold mount would stay
				// plain text. On the main thread it uses the shared highlighter the
				// parent warmed up, and conflict files are small.
				disableWorkerPool
				renderCustomHeader={() => (
					<header className={styles.fileHeader}>
						<h4 className={classes("text-13", styles.filePath)}>
							<FileIcon fileName={fileName} className={styles.icon} />
							{fileName}
							{directoryPath !== null && <span className={styles.pathInit}>{directoryPath}</span>}
						</h4>
						<Badge variant="danger">Conflicted</Badge>
					</header>
				)}
				renderMergeConflictUtility={(action) => {
					// Marker block N is hunk N+1: the marker text comes from the same
					// scan that produced `hunks`, and ambiguous files go to manual.
					const conflict = p.file.hunks[action.conflictIndex];
					if (!conflict) return null;

					return (
						<ConflictActions
							projectId={p.projectId}
							commitId={p.commitId}
							path={p.file.path}
							hunk={action.conflictIndex + 1}
							conflict={conflict}
							busy={p.busy}
							editFileOptions={p.editFileOptions}
							editFileStyle={p.style}
							onResolve={(resolution) =>
								p.onResolve([{ path: p.file.path, hunk: action.conflictIndex + 1, resolution }])
							}
						/>
					);
				}}
			/>
		</section>
	);
};

/**
 * One conflict's controls, rendered in the library's per-conflict slot beside
 * where its own actions would sit — and styled like them, since those resolve
 * the parsed view client-side while these have to rewrite the commit.
 */
const ConflictActions: FC<{
	projectId: string;
	commitId: string;
	path: string;
	/** 1-based, as the resolve API addresses conflicts. */
	hunk: number;
	conflict: ConflictHunk;
	busy: boolean;
	editFileOptions: FileProps["options"];
	editFileStyle: FileProps["style"];
	onResolve: (resolution: HunkResolution) => void;
}> = (p) => {
	const dispatch = useAppDispatch();
	const [editing, setEditing] = useState(false);
	// The editor owns its document; Apply reads it off the instance rather
	// than mirroring keystrokes into state, which would re-render the slot
	// for nothing.
	const editorRef = useRef<Editor<unknown> | null>(null);
	const conflict = { commitId: p.commitId, path: p.path, id: p.conflict.id };
	// A primitive, so checking one conflict re-renders one slot rather than all.
	const checked = useAppSelector((state) =>
		projectSlice.selectors.selectIsConflictChecked(state, p.projectId, conflict),
	);
	// A checked conflict is resolved from the bar, so its own resolutions go
	// inert — disabled, not hidden, which would shift the rows below. The
	// checkbox and Cancel stay live, or checking would trap you here.
	const disabled = p.busy || checked;

	const apply = (resolution: HunkResolution) => {
		if (disabled) return;
		p.onResolve(resolution);
	};
	const both = [p.conflict.ours, p.conflict.theirs].filter((side) => side !== "").join("\n");

	return (
		<div className={styles.actions}>
			{/* Checking hands the conflict to the batch actions, which contradicts
			    the resolution being typed right here — hidden, not disabled. */}
			{!editing && (
				<Checkbox
					className={styles.check}
					checked={checked}
					disabled={p.busy}
					aria-label={`Check conflict ${p.hunk} in ${p.path}`}
					onCheckedChange={(next) =>
						dispatch(
							projectSlice.actions.checkConflict({
								projectId: p.projectId,
								conflict,
								checked: next,
							}),
						)
					}
				/>
			)}
			{!editing ? (
				<>
					<button
						type="button"
						className={classes(styles.action, styles.actionCurrent)}
						disabled={disabled}
						onClick={() => apply({ type: "ours" })}
					>
						Accept current change
					</button>
					<span aria-hidden className={styles.actionSeparator}>
						|
					</span>
					<button
						type="button"
						className={classes(styles.action, styles.actionIncoming)}
						disabled={disabled}
						onClick={() => apply({ type: "theirs" })}
					>
						Accept incoming change
					</button>
					<span aria-hidden className={styles.actionSeparator}>
						|
					</span>
					<button
						type="button"
						className={styles.action}
						disabled={disabled}
						onClick={() => apply({ type: "content", subject: both })}
					>
						Accept both
					</button>
					<span aria-hidden className={styles.actionSeparator}>
						|
					</span>
					<button
						type="button"
						className={styles.action}
						disabled={disabled}
						onClick={() => setEditing(true)}
					>
						Edit
					</button>
				</>
			) : (
				<>
					<div className={classes(styles.hint, "text-12")}>
						This replaces the whole conflicted region. Leaving it empty deletes the region; never
						include conflict markers.
					</div>
					<div className={styles.editor}>
						<File
							file={{ name: p.path, contents: p.conflict.theirs }}
							options={p.editFileOptions}
							style={p.editFileStyle}
							edit={!disabled}
							editorOptions={{
								onAttach: (editor) => {
									editorRef.current = editor;
								},
							}}
							disableWorkerPool
						/>
					</div>
					<div className={styles.editorActions}>
						<button
							type="button"
							className={getButtonClassName({ variant: "outline", size: "small" })}
							disabled={disabled}
							onClick={() =>
								apply({
									type: "content",
									subject: editorRef.current?.getFile()?.contents ?? p.conflict.theirs,
								})
							}
						>
							Apply resolution
						</button>
						<button
							type="button"
							className={getButtonClassName({ variant: "ghost", size: "small" })}
							disabled={p.busy}
							onClick={() => setEditing(false)}
						>
							Cancel
						</button>
					</div>
				</>
			)}
		</div>
	);
};
