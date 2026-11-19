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
import { FileIcon } from "./FileIcon.tsx";
import { FileStatusBadge, type FileStatusType } from "./FileStatusBadge.tsx";
import { FolderIcon } from "./FolderIcon.tsx";
import { Icon } from "./Icon.tsx";
import { ScrollArea } from "./ScrollArea.tsx";
import { Tooltip } from "./Tooltip.tsx";
import styles from "./FileList.module.css";
import type { Guides } from "./treeGuides.ts";

const pluralRules = new Intl.PluralRules("en");
const plural = (count: number, word: string) =>
	`${count} ${word}${pluralRules.select(count) === "one" ? "" : "s"}`;

/**
 * A panel of changed files: a header naming them, with their count and line totals, over the
 * {@link FileListItem}s, which scroll under it inset from the panel's edges and a pixel apart.
 *
 * - `actions` sit at the header's end, after the search button that `onOpenFilter` adds. With
 *   `filter` given, the header is the filter field instead: typing narrows the list, Escape closes
 *   it and the down arrow hands over to the list through `onEnterList`. Its `found`, how many of
 *   the `count` files it keeps, shows at the field's end as "2 of 7" once something is typed. Opening it, the title
 *   steps aside, the field widens in, and the search button slides into the actions' place to
 *   become the close button; closing plays it back.
 * - The rows scroll in a {@link ScrollArea}, a hairline under the header once they move. Give a
 *   virtualizer `viewportRef`; a virtualised list positions its rows itself, so it takes the pixel
 *   between them from the virtualizer (`gap: 1`) rather than from the list.
 * - A selected item shows the solid fill while the list holds the selection focus, and the
 *   quieter one otherwise. Pass `focused`, or set `data-selection-focused="true"` on the element
 *   holding the rows without re-rendering; `data-selection-focus-styles="false"` further out turns
 *   the solid fill off for everything inside, as during a drag.
 * - `header` takes the header's place, for a list in a popup that hangs from a button already
 *   naming and counting its files: lead it with `PopupSearch`. The title, stats, filter and
 *   `actions` then don't show, and neither does the scroll hairline, the header drawing its own.
 *
 * @public
 * @import import { FileList, FileListItem } from "@gitbutler/but-ui-react/FileList.tsx";
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
			found?: number;
		} | null;
		onHeaderContextMenu?: (event: MouseEvent<HTMLDivElement>) => void;
		focused?: boolean;
		viewportRef?: Ref<HTMLDivElement>;
		header?: ReactNode;
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
	header,
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
	const found =
		filter?.found !== undefined && shownValue !== "" ? `${filter.found} of ${count}` : undefined;
	return (
		<div {...props} className={classes(className, styles.panel)}>
			{header ?? (
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
					<Field.Root
						render={<FieldRootStyles />}
						className={styles.filterField}
						inert={!filtering}
					>
						<FieldControlWithIcon
							// A fresh input on opening, so it takes focus.
							key={String(filtering)}
							// oxlint-disable-next-line jsx_a11y/no-autofocus
							autoFocus={filtering}
							id={filter?.inputId}
							className="text-13"
							// Room for the count, which sits over the field's end.
							style={
								found === undefined
									? undefined
									: { paddingInlineEnd: `calc(${found.length}ch + 8px)` }
							}
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
						<span className={classes("text-13", styles.filterFound)} aria-live="polite">
							{found}
						</span>
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
			)}
			{/* A header passed in, such as a popup's search, ends on its own hairline. */}
			<ScrollArea
				separator={header === undefined}
				className={styles.body}
				viewportRef={viewportRef}
			>
				<div
					className={classes(styles.list, header !== undefined && styles.listOwnTop)}
					data-selection-focused={focused ? "true" : undefined}
				>
					{children}
				</div>
			</ScrollArea>
		</div>
	);
};

type Ranges = ReadonlyArray<readonly [start: number, end: number]>;

/** `text` with the letters `found` covers underlined; `offset` is where `text` starts in what was
 * matched. */
const marked = (text: string, found: Ranges | undefined, offset = 0): ReactNode => {
	if (found === undefined || found.length === 0) return text;
	const parts: Array<ReactNode> = [];
	let at = 0;
	for (const [start, end] of found) {
		const [from, to] = [Math.max(start - offset, at), Math.min(end - offset, text.length)];
		if (to <= from) continue;
		if (from > at) parts.push(text.slice(at, from));
		parts.push(
			<span key={from} className={styles.match}>
				{text.slice(from, to)}
			</span>,
		);
		at = to;
	}
	if (at < text.length) parts.push(text.slice(at));
	return parts;
};

/** A row's folder, cut in the middle where the row is short of room, at the characters the GPUI
 * client would cut it at. */
const Directory: FC<{ text: string; found: Ranges | undefined; trail?: boolean }> = ({
	text,
	found,
	trail = false,
}) => {
	// Where the cut starts and ends in `text`, while it doesn't fit.
	const [cut, setCut] = useState<readonly [number, number] | null>(null);
	const fit = (box: HTMLSpanElement | null) => {
		const copy = box?.querySelector<HTMLElement>("[data-copy]");
		const ellipsis = box?.querySelector<HTMLElement>("[data-ellipsis]");
		const node = copy?.firstChild;
		if (!box || !copy || !ellipsis || !node) return;
		const range = document.createRange();
		const until = (index: number) => {
			range.setStart(node, 0);
			range.setEnd(node, index);
			return range.getBoundingClientRect().width;
		};
		const place = () => {
			const total = until(text.length);
			// Asks for the whole folder's width, so the room it's given doesn't shrink with the cut.
			box.style.width = `${total}px`;
			const room = box.getBoundingClientRect().width;
			if (total <= room) return setCut(null);
			// As GPUI cuts it: two thirds of the room to the start, the rest to the end, in whole
			// characters.
			const budget = room - ellipsis.getBoundingClientRect().width;
			let front = 0;
			while (front < text.length && until(front + 1) <= (budget * 2) / 3) front++;
			const back = budget - until(front);
			let from = text.length;
			while (from > front && total - until(from - 1) <= back) from--;
			setCut((was) => (was?.[0] === front && was[1] === from ? was : [front, from]));
		};
		place();
		// The copy too: it widens when the web font arrives.
		const observer = new ResizeObserver(place);
		observer.observe(box);
		observer.observe(copy);
		return () => observer.disconnect();
	};
	return (
		<span ref={fit} className={classes(styles.directory, trail && styles.directoryTrail)}>
			{cut === null ? (
				marked(text, found)
			) : (
				<>
					{marked(text.slice(0, cut[0]), found)}…{marked(text.slice(cut[1]), found, cut[1])}
				</>
			)}
			<span data-copy aria-hidden className={styles.directoryCopy}>
				{text}
			</span>
			<span data-ellipsis aria-hidden className={styles.directoryCopy}>
				…
			</span>
		</span>
	);
};

/**
 * One changed file, or a directory of them, in a {@link FileList}: its icon, its name, and what
 * happened to it. The row is the host's to wire: `render` it as a tree item or an option, and
 * give it `onClick`, `inert` while it can't be acted on, and a `ref` and `style` when virtualised.
 *
 * - `directory` is the path to the file, shown faded after its name, or before it with
 *   `directoryPosition="lead"`; leave it out where a tree already says where the file is. Short of
 *   room, the folder gives way before the name and is cut in the middle, so its end, which tells
 *   two folders apart, stays.
 * - `matches` underlines what a filter found, as `[start, end)` offsets into `name` and into
 *   `directory`, so it's clear why a row is listed.
 * - `depth` indents the row one step per directory it sits in, each step drawing the guide that
 *   ties it to its directory, so a tree reads as one; `guides`, from `treeGuides`, end each
 *   directory's guide with a tail into its last row. A directory row sets `folded`, which gives
 *   it a folder and a chevron in a step of its own before it, as on a sidebar row; `onToggleFolded`
 *   answers the chevron, and `toggleRender` wraps it, for a tooltip trigger.
 * - `icon` defaults to the file's type glyph, or a folder for a directory.
 * - `checkbox` shares the icon's cell and takes its place once the pointer is over it or it is
 *   checked, so a row stays quiet while browsing. A directory's stands for every file under it,
 *   `indeterminate` while only some are checked.
 * - `inert` while an action on the file runs, or while it can't be acted on: the row fades and
 *   takes no clicks. It shows no spinner of its own: one snackbar says what's running, once for
 *   all the files it touches (but-ui's Toasts and snackbars note).
 * - The last cell says what happened: the warning in amber if it is `conflicted`, as on a
 *   conflicted commit, else a pencil if
 *   it is `editing`, open to edit with nothing changed yet, else a tick if it is `reviewed`, which
 *   also fades the name (a reviewed file's news is that it is done with), else the letter for its
 *   `status`, else a folded directory's `count` of files.
 * - Selected rows next to one another read as one block, as Finder draws a picked run: rounded
 *   only where it starts and ends, with a faint line between two of its rows.
 * - The row has no ⋯: its menu opens with a right-click, the row a `ContextMenu`'s `trigger`, so
 *   the last cell always says what happened and the letters read down the list. `marks` show
 *   before it.
 * - `added` and `removed` give the file's line counts, after its name; either side shows only
 *   when it isn't zero. A directory's, and its `count`, show only while it's folded: open, its
 *   files say it.
 * - `labelRender` and `statusRender` wrap the name and the status, for tooltip triggers.
 *
 * @figma `state` is the drawing's only: hover is the row under the pointer, which the code draws
 * from CSS. `line counts` shows `added` and `removed`; edit the numbers on the DiffStats inside.
 * With `editing` on, turn `status` off: the pencil takes the letter's place.
 * @public
 * @import import { FileList, FileListItem } from "@gitbutler/but-ui-react/FileList.tsx";
 */
export const FileListItem: FC<
	{
		name: string;
		directory?: string | null;
		directoryPosition?: "lead" | "trail";
		/** What a filter found, as `[start, end)` offsets into `name` and `directory`, underlined. */
		matches?: { name?: Ranges; directory?: Ranges };
		icon?: ReactNode;
		checkbox?: ReactNode;
		depth?: number;
		/** How each level's guide draws on this row, outermost first: work them out with
		 * `treeGuides` (treeGuides.ts) from the rows the tree shows. */
		guides?: Guides;
		folded?: boolean;
		onToggleFolded?: () => void;
		toggleRender?: ReactElement;
		status?: FileStatusType;
		count?: number;
		reviewed?: boolean;
		conflicted?: boolean;
		editing?: boolean;
		selected?: boolean;
		marks?: ReactNode;
		added?: number;
		removed?: number;
		labelRender?: ReactElement;
		statusRender?: ReactElement;
	} & useRender.ComponentProps<"div">
> = ({
	name,
	directory,
	directoryPosition = "trail",
	matches,
	icon,
	checkbox,
	depth = 0,
	guides,
	folded,
	onToggleFolded,
	toggleRender,
	status,
	count,
	reviewed = false,
	conflicted = false,
	editing = false,
	selected = false,
	marks,
	added = 0,
	removed = 0,
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
						<Directory text={`${directory}/`} found={matches?.directory} />
					)}
					<span className={styles.name}>{marked(name, matches?.name)}</span>
					{directory != null && directoryPosition === "trail" && (
						<Directory text={directory} found={matches?.directory} trail />
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

	// An open directory's files say what happened to them, so its own totals wait until it folds.
	const open = folded === false;
	const shownCount = open ? undefined : count;
	const hasStatus =
		conflicted || editing || reviewed || status !== undefined || shownCount !== undefined;
	const statusMark = useRender({
		render: statusRender ?? <span />,
		props: {
			className: styles.status,
			children: conflicted ? (
				// oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A glyph with a name, not a picture with a source.
				<span role="img" aria-label="Conflicted" className={styles.conflicted}>
					<Icon name="warning" size={14} />
				</span>
			) : editing ? (
				<span aria-label="Editing" className={styles.editing}>
					<Icon name="edit" />
				</span>
			) : reviewed ? (
				<span aria-label="Reviewed" className={styles.reviewed}>
					<Icon size={11} name="tick" />
				</span>
			) : status !== undefined ? (
				<FileStatusBadge status={status} />
			) : shownCount !== undefined ? (
				<span className={classes("text-12", styles.count)}>{shownCount}</span>
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
								<span
									key={level}
									className={classes(
										styles.step,
										styles.guide,
										guides?.[level] === "last" && styles.guideLast,
										guides?.[level] === "none" && styles.guideNone,
									)}
									aria-hidden
								/>
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
					<DiffStats
						added={open ? 0 : added}
						removed={open ? 0 : removed}
						className={classes("text-12", styles.lineStats, reviewed && styles.faded)}
					/>
					{marks !== undefined && <span className={styles.marks}>{marks}</span>}
					{/* Reserved on every row, so what comes before it lines up down the list. */}
					<span className={styles.end}>{hasStatus && statusMark}</span>
				</>
			),
		}),
	});
};
