import { mergeProps, useRender } from "@base-ui/react";
import type { FC, ReactElement, ReactNode } from "react";
import { classes } from "./classes.ts";
import { ConflictIcon } from "./ConflictIcon.tsx";
import { FileIcon } from "./FileIcon.tsx";
import { FileStatusBadge, type FileStatusType } from "./FileStatusBadge.tsx";
import { FolderIcon } from "./FolderIcon.tsx";
import { Icon } from "./Icon.tsx";
import styles from "./FileList.module.css";

/**
 * A list of changed files: {@link FileListItem}s inset from the list's edges, a pixel apart. The
 * host gives it its role, as `tree` or `listbox`.
 *
 * A selected item shows the solid fill while the list holds the selection focus, and the quieter
 * one otherwise. Pass `focused`, or set `data-selection-focused="true"` on the list or an element
 * around it without re-rendering; `data-selection-focus-styles="false"` further out turns the
 * solid fill off for everything inside, as during a drag.
 *
 * A virtualised list positions its items itself, so it gives its virtualizer the pixel between
 * them (`gap: 1`) rather than relying on this list's gap.
 *
 * @public
 * @import import { FileList, FileListItem } from "@gitbutler/ui-react/FileList.tsx";
 */
export const FileList: FC<{ focused?: boolean } & useRender.ComponentProps<"div">> = ({
	focused,
	render,
	...props
}) =>
	useRender({
		render: render ?? <div />,
		state: { focused: focused === true },
		stateAttributesMapping: {
			focused: (value) => (value ? { "data-selection-focused": "true" } : null),
		},
		props: mergeProps<"div">(props, { className: styles.list }),
	});

/**
 * One changed file, or a directory of them, in a {@link FileList}: its icon, its name, and what
 * happened to it. The row is the host's to wire: `render` it as a tree item or an option, and
 * give it `onClick`, `inert` while it can't be acted on, and a `ref` and `style` when virtualised.
 *
 * - `directory` is the path to the file, shown faded after its name, or before it with
 *   `directoryPosition="lead"`; leave it out where a tree already says where the file is.
 * - `depth` indents the row one step per directory it sits in, each step drawing the guide that
 *   ties it to its directory, so a tree reads as one. A directory row sets `folded`, which gives
 *   it a folder and a chevron in its own step; `onToggleFolded` answers the chevron, and
 *   `toggleRender` wraps it, for a tooltip trigger.
 * - `icon` defaults to the file's type glyph, or a folder for a directory.
 * - `checkbox` shares the icon's cell and takes its place once the pointer is over it or it is
 *   checked, so a row stays quiet while browsing.
 * - The last cell says what happened: the conflict mark if it is `conflicted`, else a tick if it
 *   is `reviewed`, which also fades the name (a reviewed file's news is that it is done with),
 *   else the letter for its `status`, else a folded directory's `count` of files.
 * - `actions` take the status's place while the row is hovered, holds focus or is selected, so a
 *   keyboard user finds them where the pointer would; both sit centred in one fixed cell, so
 *   nothing moves. `marks` always show, before it.
 * - `labelRender` and `statusRender` wrap the name and the status, for tooltip triggers.
 *
 * @public
 * @import import { FileList, FileListItem } from "@gitbutler/ui-react/FileList.tsx";
 */
export const FileListItem: FC<
	{
		name: string;
		directory?: string | null;
		directoryPosition?: "lead" | "trail";
		icon?: ReactNode;
		checkbox?: ReactNode;
		depth?: number;
		folded?: boolean;
		onToggleFolded?: () => void;
		toggleRender?: ReactElement;
		status?: FileStatusType;
		count?: number;
		reviewed?: boolean;
		conflicted?: boolean;
		selected?: boolean;
		actions?: ReactNode;
		marks?: ReactNode;
		labelRender?: ReactElement;
		statusRender?: ReactElement;
	} & useRender.ComponentProps<"div">
> = ({
	name,
	directory,
	directoryPosition = "trail",
	icon,
	checkbox,
	depth = 0,
	folded,
	onToggleFolded,
	toggleRender,
	status,
	count,
	reviewed = false,
	conflicted = false,
	selected = false,
	actions,
	marks,
	labelRender,
	statusRender,
	render,
	...props
}) => {
	const label = useRender({
		render: labelRender ?? <div />,
		props: {
			className: classes(styles.label, reviewed && styles.faded),
			children: (
				<span className={classes("text-13", styles.text)}>
					{directory != null && directoryPosition === "lead" && (
						<span className={styles.directory}>{directory}/</span>
					)}
					{name}
					{directory != null && directoryPosition === "trail" && (
						<span className={classes(styles.directory, styles.directoryTrail)}>{directory}</span>
					)}
				</span>
			),
		},
	});

	const toggleLabel = `${folded === true ? "Expand" : "Collapse"} ${name}`;
	const toggle = useRender({
		render: toggleRender ?? <button type="button" aria-label={toggleLabel} />,
		props: {
			// A tree moves with the arrow keys rather than Tab, so the chevron stays out of the tab
			// order and takes the click on its own, leaving the rest of the row to select.
			tabIndex: -1,
			"aria-label": toggleLabel,
			"aria-expanded": folded !== true,
			className: classes(styles.step, styles.toggle),
			onClick: onToggleFolded,
			children: <Icon size={12} name={folded === true ? "chevron-right" : "chevron-down"} />,
		},
	});

	const hasStatus = conflicted || reviewed || status !== undefined || count !== undefined;
	const statusMark = useRender({
		render: statusRender ?? <span />,
		props: {
			className: styles.status,
			children: conflicted ? (
				<ConflictIcon variant="conflict" size={14} aria-label="Conflicted" />
			) : reviewed ? (
				<span aria-label="Reviewed" className={styles.reviewed}>
					<Icon size={11} name="tick" />
				</span>
			) : status !== undefined ? (
				<FileStatusBadge status={status} />
			) : count !== undefined ? (
				<span className={classes("text-11", styles.count)}>{count}</span>
			) : null,
		},
	});

	return useRender({
		render: render ?? <div />,
		state: { selected },
		props: mergeProps<"div">(props, {
			className: styles.item,
			children: (
				<>
					{(depth > 0 || folded !== undefined) && (
						<span className={styles.steps}>
							{Array.from({ length: depth }, (_, level) => (
								<span key={level} className={classes(styles.step, styles.guide)} aria-hidden />
							))}
							{folded !== undefined && toggle}
						</span>
					)}
					<span className={styles.lead}>
						<span className={classes(styles.icon, reviewed && styles.faded)}>
							{icon ?? (folded !== undefined ? <FolderIcon /> : <FileIcon fileName={name} />)}
						</span>
						{checkbox !== undefined && <span className={styles.checkbox}>{checkbox}</span>}
					</span>
					{label}
					{marks !== undefined && <span className={styles.marks}>{marks}</span>}
					{/* Reserved on every row, so what comes before it lines up down the list. */}
					<span className={styles.end}>
						{hasStatus && statusMark}
						{actions !== undefined && <span className={styles.actions}>{actions}</span>}
					</span>
				</>
			),
		}),
	});
};
