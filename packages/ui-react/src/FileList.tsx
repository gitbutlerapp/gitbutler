import { Field, mergeProps, useRender } from "@base-ui/react";
import {
	useState,
	type ComponentProps,
	type FC,
	type MouseEvent,
	type ReactElement,
	type ReactNode,
	type Ref,
} from "react";
import { Badge } from "./Badge.tsx";
import { Button } from "./Button.tsx";
import { classes } from "./classes.ts";
import { DiffStats } from "./DiffStats.tsx";
import { FieldControlWithIcon, FieldRootStyles } from "./Field.tsx";
import { ConflictIcon } from "./ConflictIcon.tsx";
import { FileIcon } from "./FileIcon.tsx";
import { FileStatusBadge, type FileStatusType } from "./FileStatusBadge.tsx";
import { FolderIcon } from "./FolderIcon.tsx";
import { Icon } from "./Icon.tsx";
import { ScrollArea } from "./ScrollArea.tsx";
import { Tooltip } from "./Tooltip.tsx";
import styles from "./FileList.module.css";

const pluralRules = new Intl.PluralRules("en");
const plural = (count: number, word: string) =>
	`${count} ${word}${pluralRules.select(count) === "one" ? "" : "s"}`;

/**
 * A panel of changed files: a header naming them, with their count and line totals, over the
 * {@link FileListItem}s, which scroll under it inset from the panel's edges and a pixel apart.
 *
 * - `actions` sit at the header's end, after the search button that `onOpenFilter` adds. With
 *   `filter` given, the header is the filter field instead: typing narrows the list, Escape closes
 *   it and the down arrow hands over to the list through `onEnterList`. Opening it, the title
 *   steps aside, the field widens in, and the search button slides into the actions' place to
 *   become the close button; closing plays it back.
 * - The rows scroll in a {@link ScrollArea}, a hairline under the header once they move. Give a
 *   virtualizer `viewportRef`; a virtualised list positions its rows itself, so it takes the pixel
 *   between them from the virtualizer (`gap: 1`) rather than from the list.
 * - A selected item shows the solid fill while the list holds the selection focus, and the
 *   quieter one otherwise. Pass `focused`, or set `data-selection-focused="true"` on the element
 *   holding the rows without re-rendering; `data-selection-focus-styles="false"` further out turns
 *   the solid fill off for everything inside, as during a drag.
 *
 * @public
 * @import import { FileList, FileListItem } from "@gitbutler/ui-react/FileList.tsx";
 */
export const FileList: FC<
	{
		title: string;
		count: number;
		added?: number;
		removed?: number;
		actions?: ReactNode;
		onOpenFilter?: () => void;
		filter?: {
			value: string;
			onChange: (value: string) => void;
			onClose: () => void;
			onEnterList?: () => void;
			inputId?: string;
		} | null;
		onHeaderContextMenu?: (event: MouseEvent<HTMLDivElement>) => void;
		focused?: boolean;
		viewportRef?: Ref<HTMLDivElement>;
	} & ComponentProps<"div">
> = ({
	title,
	count,
	added = 0,
	removed = 0,
	actions,
	onOpenFilter,
	filter,
	onHeaderContextMenu,
	focused = false,
	viewportRef,
	className,
	children,
	...props
}) => {
	const described = [
		`${plural(count, "file")} changed`,
		...(added > 0 ? [`${plural(added, "line")} added`] : []),
		...(removed > 0 ? [`${plural(removed, "line")} removed`] : []),
	];
	const filtering = filter != null;
	// Both faces stay mounted so they can trade places, and the field keeps showing what was typed
	// while it fades out.
	const [shownValue, setShownValue] = useState(filter?.value ?? "");
	if (filter && filter.value !== shownValue) setShownValue(filter.value);
	return (
		<div {...props} className={classes(className, styles.panel)}>
			<div
				className={classes(styles.header, filtering && styles.filtering)}
				onContextMenu={filtering ? undefined : onHeaderContextMenu}
			>
				<span className={styles.label} inert={filtering}>
					<span className={classes("text-14", "text-bold", styles.title)}>{title}</span>
					<Tooltip content={described[0]}>
						<span className={styles.stats} aria-label={described.join(", ")}>
							<Badge variant="lightGray">{count}</Badge>
							<DiffStats added={added} removed={removed} className="text-12" />
						</span>
					</Tooltip>
				</span>
				<Field.Root render={<FieldRootStyles />} className={styles.filterField} inert={!filtering}>
					<FieldControlWithIcon
						// A fresh input on opening, so it takes focus.
						key={String(filtering)}
						// oxlint-disable-next-line jsx_a11y/no-autofocus
						autoFocus={filtering}
						id={filter?.inputId}
						className="text-13"
						icon={<Icon name="search" />}
						aria-label="Filter files"
						placeholder="Filter files"
						value={shownValue}
						onChange={(event) => filter?.onChange(event.currentTarget.value)}
						onKeyDown={(event) => {
							if (!filter) return;
							if (event.key === "Escape") {
								event.preventDefault();
								event.stopPropagation();
								filter.onClose();
							} else if (event.key === "ArrowDown" && filter.onEnterList) {
								event.preventDefault();
								event.stopPropagation();
								filter.onEnterList();
							}
						}}
					/>
				</Field.Root>
				<span className={styles.headerEnd}>
					{(filter || (onOpenFilter && count > 0)) && (
						<Button
							variant="ghost"
							iconOnly
							aria-label={filter ? "Close files filter" : "Filter files"}
							onClick={filter ? filter.onClose : onOpenFilter}
						>
							{/* The magnifier becomes the cross: the icon crossfade from motion.md. */}
							<span className={styles.toggleIcons}>
								{/* Wrapped, so the fade isn't overruled by the button's own icon opacity. */}
								<span className={classes(styles.toggleIcon, filtering && styles.toggleIconGone)}>
									<Icon name="search" />
								</span>
								<span className={classes(styles.toggleIcon, !filtering && styles.toggleIconGone)}>
									<Icon name="cross" />
								</span>
							</span>
						</Button>
					)}
					{actions !== undefined && (
						<span className={styles.headerActions} inert={filtering}>
							<span className={styles.headerActionsInner}>{actions}</span>
						</span>
					)}
				</span>
			</div>
			<ScrollArea separator className={styles.body} viewportRef={viewportRef}>
				<div className={styles.list} data-selection-focused={focused ? "true" : undefined}>
					{children}
				</div>
			</ScrollArea>
		</div>
	);
};

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
