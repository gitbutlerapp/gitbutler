import { mergeProps, useRender } from "@base-ui/react";
import type { ComponentProps, FC, ReactNode } from "react";
import { Badge, type BadgeVariant } from "./Badge.tsx";
import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import { MetaCount } from "./MetaCount.tsx";
import styles from "./SidebarRow.module.css";
import { Tooltip } from "./Tooltip.tsx";

/**
 * How much a sidebar row says. `compact` is one line with its state at the right edge, where it
 * stays: the menu follows it under the pointer, and the actions take its place. `rich` keeps the
 * state and the actions in view: right after the name on a one-line row, or on a second line under
 * a pull request's title.
 */
export type SidebarRowLayout = "compact" | "rich";

/** What every sidebar row takes. */
type RowProps = {
	/** How many levels the row sits below the top of the card, one guide per level. */
	depth?: number;
	/** Gives the row a chevron: `true` while its children are hidden. Leave it out on a leaf. */
	folded?: boolean;
	onToggleFolded?: () => void;
	/** Selected: the quiet fill, or the solid one while the list holds the selection focus. */
	selected?: boolean;
	/** `MetaCount`s and a `CiStatus`, in the order the row should read them. */
	meta?: ReactNode;
	/** The row's next step, as small outline `Button`s: Push, Update, Create PR, View PR. In
	 * `compact` they take the state's place under the pointer and on keyboard focus; leave them out
	 * when there is none, and the row keeps its state there. */
	actions?: ReactNode;
	/** The row's menu button, a small ghost icon-only `Button`. It shows under the pointer, on
	 * keyboard focus and on a selected row. */
	menu?: ReactNode;
	/** What the name stands for in full (a path, a pull request's state, a commit's author), shown
	 * over the name alone, so the counts beside it keep their own. */
	tooltip?: ReactNode;
} & useRender.ComponentProps<"div">;

const Steps: FC<{
	depth: number;
	folded: boolean | undefined;
	onToggleFolded: (() => void) | undefined;
	name: string;
}> = ({ depth, folded, onToggleFolded, name }) =>
	depth > 0 || folded !== undefined ? (
		<span className={styles.steps}>
			{Array.from({ length: depth }, (_, level) => (
				<span key={level} className={classes(styles.step, styles.guide)} aria-hidden />
			))}
			{folded !== undefined && (
				// A tree moves with the arrow keys, so the chevron stays out of the tab order and takes
				// its click on its own, leaving the rest of the row to open what it names.
				<button
					type="button"
					tabIndex={-1}
					aria-label={`${folded ? "Expand" : "Collapse"} ${name}`}
					aria-expanded={!folded}
					className={classes(styles.step, styles.toggle)}
					onClick={(event) => {
						event.stopPropagation();
						event.preventDefault();
						onToggleFolded?.();
					}}
				>
					<Icon size={12} name={folded ? "chevron-right" : "chevron-down"} />
				</button>
			)}
		</span>
	) : null;

/** A row's name, with its `tooltip` on the text itself rather than on the whole row. */
const Name: FC<{ tooltip: ReactNode; className: string; children: ReactNode }> = ({
	tooltip,
	className,
	children,
}) => {
	const name = <span className={className}>{children}</span>;
	return tooltip === undefined ? (
		name
	) : (
		<Tooltip content={tooltip} side="bottom">
			{name}
		</Tooltip>
	);
};

/** A slot the host filled with nothing (`null`, `false`) is no slot: the row keeps the room and
 * the state it would otherwise give up for it. */
const filled = (slot: ReactNode) => (slot === null || slot === false ? undefined : slot);

/** The one-line row every sidebar item but a pull request in `rich` is drawn on. */
const Row: FC<
	RowProps & {
		name: string;
		icon: ReactNode;
		label: ReactNode;
		layout: SidebarRowLayout;
		rowClassName?: string;
	}
> = ({
	name,
	icon,
	label,
	layout,
	depth = 0,
	folded,
	onToggleFolded,
	selected = false,
	meta: metaSlot,
	actions: actionsSlot,
	menu: menuSlot,
	rowClassName,
	render,
	...props
}) => {
	const [meta, actions, menu] = [filled(metaSlot), filled(actionsSlot), filled(menuSlot)];
	return useRender({
		render: render ?? <div />,
		state: { selected },
		props: mergeProps<"div">(props, {
			className: classes(styles.row, styles[layout], rowClassName),
			children: (
				<>
					<Steps depth={depth} folded={folded} onToggleFolded={onToggleFolded} name={name} />
					<span className={styles.icon}>{icon}</span>
					<span className={styles.label}>{label}</span>
					{layout === "compact" ? (
						// The state and the actions share the row's end, so neither moves when one gives way
						// to the other.
						(meta !== undefined || actions !== undefined) && (
							<span
								className={classes(
									styles.end,
									actions !== undefined && (meta === undefined ? styles.endBare : styles.endSwaps),
								)}
							>
								{meta !== undefined && <span className={styles.meta}>{meta}</span>}
								{actions !== undefined && <span className={styles.hover}>{actions}</span>}
							</span>
						)
					) : (
						<>
							{meta !== undefined && <span className={styles.meta}>{meta}</span>}
							{actions !== undefined && <span className={styles.actions}>{actions}</span>}
						</>
					)}
					{menu !== undefined && <span className={styles.menu}>{menu}</span>}
				</>
			),
		}),
	});
};

/**
 * A machine's online light: lit green while it is connected, grey once it isn't. Say when it was
 * last seen in a `Tooltip` around it.
 *
 * @import import { StatusLed } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const StatusLed: FC<{ online: boolean } & ComponentProps<"span">> = ({
	online,
	...props
}) => (
	<span
		// oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A drawn dot with a name, not a picture with a source.
		role="img"
		aria-label={online ? "Online" : "Offline"}
		{...props}
		className={classes(props.className, styles.led, online && styles.online)}
	/>
);

/**
 * A machine in the sidebar: the card's header when the sidebar is grouped by machine, a row
 * inside a repository when grouped by repository. Its picture is the host's (`icon`, 14px); an
 * offline machine's dims, and its light goes grey.
 *
 * Folded, its `meta` sums up what it holds (`repos`, or `worktrees` when grouped by repository)
 * and its age. Open as a card's header, it shows no age: its age is the newest of the rows under
 * it, which show their own. It is always `compact`.
 *
 * @import import { MachineItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const MachineItem: FC<
	{ name: string; icon: ReactNode; online: boolean } & Omit<RowProps, "layout">
> = ({ name, icon, online, tooltip, ...props }) => (
	<Row
		{...props}
		name={name}
		layout="compact"
		icon={<span className={classes(styles.picture, !online && styles.offline)}>{icon}</span>}
		label={
			<>
				<Name tooltip={tooltip} className={classes("text-14", "text-semibold", styles.name)}>
					{name}
				</Name>
				<StatusLed online={online} />
			</>
		}
	/>
);

/**
 * A repository in the sidebar. Its `meta` says what the rows under it don't: open, how far a
 * single worktree is `behind` and its age (its uncommitted files are an {@link UncommittedItem},
 * several worktrees are rows of their own with their own ages); folded, the full summary, its
 * `worktrees` when it has several, and its age. Open as a card's header, it shows no age, as
 * {@link MachineItem} doesn't. In `rich`, the counts follow its name and Update follows them
 * whenever it is behind.
 * Folded, it is a summary and reads as `compact` in either layout: the whole summary and Update
 * don't fit after a name at sidebar width.
 *
 * `icon` defaults to the repository glyph; a host can pass the repository's own avatar.
 *
 * @import import { RepoItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const RepoItem: FC<
	{ name: string; icon?: ReactNode; layout?: SidebarRowLayout } & RowProps
> = ({ name, icon, layout = "compact", tooltip, ...props }) => (
	<Row
		{...props}
		name={name}
		layout={props.folded ? "compact" : layout}
		icon={icon ?? <Icon name="repo" />}
		label={
			<Name tooltip={tooltip} className={classes("text-14", "text-semibold", styles.name)}>
				{name}
			</Name>
		}
	/>
);

/**
 * A worktree inside a repository that has several. The `main` worktree is the repository's own
 * checkout: it comes first and takes the plain folder; the linked worktrees take the folder-copy
 * glyph. When its folder has the repository's name, call it "Main worktree" rather than repeat
 * the name, and keep the path in a `Tooltip`. It is styled like any other worktree, so the words
 * and the glyph tell it apart rather than a fainter name.
 *
 * Folded, it reads as `compact` in either layout, as {@link RepoItem} does.
 *
 * @import import { WorktreeItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const WorktreeItem: FC<
	{ name: string; main?: boolean; layout?: SidebarRowLayout } & RowProps
> = ({ name, main = false, layout = "compact", tooltip, ...props }) => (
	<Row
		{...props}
		name={name}
		layout={props.folded ? "compact" : layout}
		icon={<Icon name={main ? "folder" : "folder-copy"} />}
		label={
			<Name tooltip={tooltip} className={classes("text-14", "text-semibold", styles.name)}>
				{name}
			</Name>
		}
	/>
);

/** A pull request's state, as a {@link BranchItem} draws it. */
export type PullRequestState = "draft" | "open" | "closed" | "merged";

const prGlyph: Record<PullRequestState, IconName> = {
	draft: "pr-draft",
	open: "pr",
	closed: "pr-close",
	merged: "pr",
};

const prBadge: Record<PullRequestState, BadgeVariant> = {
	draft: "lightGray",
	open: "safe",
	closed: "danger",
	merged: "purple",
};

/**
 * A branch in the sidebar. With a pull request (`pr`), the row is the request: its state in the
 * glyph and the badge, its number, and its title as `title`; without one, `title` is the branch's
 * name.
 *
 * Its `meta` reads in one order: `CiStatus`, then ↓ `behind`, ↑ `unpushed`, `uncommitted`, and
 * `commits`. Show ↑ or ⟜, never both; on an expanded branch drop ⟜, since its commits are listed
 * under it. `actions` hold the one next step (Resolve, Update, Push, Create PR or Merge), then View
 * PR on a branch with a request.
 *
 * In `rich`, an open or draft request takes two lines: its title, which wraps once and then ends in
 * an ellipsis, over the branch's name (`branch`), its state and its actions. A plain branch stays
 * one line, its state and actions right after its name, and the name gives way first. A merged or
 * closed request is done with: one muted line in either layout.
 *
 * @import import { BranchItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const BranchItem: FC<
	{
		title: string;
		pr?: { number: number; state: PullRequestState };
		/** The branch's name, under a pull request's title in `rich`. */
		branch?: string;
		layout?: SidebarRowLayout;
	} & RowProps
> = ({ title, pr, branch, layout = "compact", tooltip, ...props }) => {
	const done = pr?.state === "merged" || pr?.state === "closed";
	const glyph = (
		<Icon
			name={pr ? prGlyph[pr.state] : "branch"}
			className={classes(styles.branchGlyph, pr && styles[pr.state])}
		/>
	);
	const badge = pr && (
		<Badge variant={prBadge[pr.state]} className={styles.badge}>
			{`#${pr.number}`}
		</Badge>
	);

	if (layout === "rich" && pr && !done)
		return <TwoLineBranch {...props} {...{ title, glyph, badge, branch, tooltip }} />;

	return (
		<Row
			{...props}
			name={title}
			layout={done ? "compact" : layout}
			rowClassName={classes(pr?.state === "merged" && styles.merged)}
			icon={glyph}
			label={
				<>
					{badge}
					<Name
						tooltip={tooltip}
						className={classes("text-14", "text-semibold", styles.name, styles.title)}
					>
						{title}
					</Name>
				</>
			}
		/>
	);
};

/** An open or draft pull request in `rich`: its title over its branch, state and actions. */
const TwoLineBranch: FC<
	RowProps & { title: string; glyph: ReactNode; badge: ReactNode; branch: string | undefined }
> = ({
	title,
	glyph,
	badge,
	branch,
	depth = 0,
	folded,
	onToggleFolded,
	selected = false,
	meta,
	actions: actionsSlot,
	menu: menuSlot,
	tooltip,
	render,
	...props
}) => {
	const [actions, menu] = [filled(actionsSlot), filled(menuSlot)];
	return useRender({
		render: render ?? <div />,
		state: { selected },
		props: mergeProps<"div">(props, {
			className: classes(styles.row, styles.rich, styles.twoLine),
			children: (
				<>
					<Steps depth={depth} folded={folded} onToggleFolded={onToggleFolded} name={title} />
					<span className={styles.content}>
						{/* Inline, so a title that wraps carries on under the glyph, as text does. */}
						<span className={classes("text-14", "text-semibold", styles.line1)}>
							<span className={styles.lead}>
								{glyph}
								{badge}
							</span>
							{tooltip === undefined ? (
								title
							) : (
								<Tooltip content={tooltip} side="bottom">
									<span>{title}</span>
								</Tooltip>
							)}
						</span>
						<span className={styles.line2}>
							{branch !== undefined && (
								<MetaCount type="branch" className={styles.branch}>
									{branch}
								</MetaCount>
							)}
							{meta}
							{actions !== undefined && <span className={styles.actions}>{actions}</span>}
						</span>
					</span>
					{menu !== undefined && (
						<span className={classes(styles.menu, styles.corner)}>{menu}</span>
					)}
				</>
			),
		}),
	});
};

/**
 * A commit under an expanded branch: the hollow glyph while it is only here, the solid one once it
 * is pushed. Its `menu` shows under the pointer, on keyboard focus and when selected.
 *
 * @import import { CommitItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const CommitItem: FC<{ message: string; pushed?: boolean } & RowProps> = ({
	message,
	pushed = true,
	tooltip,
	...props
}) => (
	<Row
		{...props}
		name={message}
		layout="compact"
		icon={<Icon name={pushed ? "commit-fill" : "commit"} />}
		label={
			<Name tooltip={tooltip} className={classes("text-13", styles.name)}>
				{message}
			</Name>
		}
	/>
);

/**
 * A worktree's uncommitted changes, as a row of its own, with how many files it has changed
 * (`count`). It sits where the work does, first among the worktree's branches:
 * under the worktree, or under a repository with only one. It opens the changes themselves, as a
 * branch opens its commits; the worktree's own row opens its overview. Leave it out while the
 * worktree is clean. Its `menu` shows under the pointer, on keyboard focus and when selected. In
 * `rich` its count follows the label, as the rows around it do.
 *
 * @import import { UncommittedItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const UncommittedItem: FC<
	{ count: number; layout?: SidebarRowLayout } & Omit<
		RowProps,
		"folded" | "onToggleFolded" | "meta" | "actions"
	>
> = ({ count, layout = "compact", tooltip, ...props }) => (
	<Row
		{...props}
		name="Uncommitted changes"
		layout={layout}
		icon={<Icon name="diff" />}
		label={
			<Name tooltip={tooltip} className={classes("text-14", "text-semibold", styles.name)}>
				Uncommitted changes
			</Name>
		}
		meta={<MetaCount type={count > 0 ? "uncommitted" : "clean"}>{count}</MetaCount>}
	/>
);

/**
 * The label over a stack of two or more branches, "Stack of 2". It doesn't open or fold; its
 * `menu` (push the stack, open its pull requests) shows under the pointer. `first` is for a stack
 * that opens right under its repository or worktree; otherwise it follows a
 * {@link WorktreeDivider} and sits tucked under it.
 *
 * @import import { StackCaption } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const StackCaption: FC<
	{ label: string; first?: boolean } & Pick<RowProps, "depth" | "menu" | "tooltip"> &
		ComponentProps<"div">
> = ({ label, first = false, depth = 0, menu, tooltip, ...props }) => (
	<Row
		{...props}
		name={label}
		layout="compact"
		depth={depth}
		menu={menu}
		rowClassName={classes(styles.caption, !first && styles.captionTucked)}
		icon={<Icon name="stack" />}
		label={
			<Name tooltip={tooltip} className={classes("text-13", styles.name)}>
				{label}
			</Name>
		}
	/>
);

/**
 * The last row of a long list, which shows twelve at a time: "12 more" opens the next page, and
 * once everything shows it reads "Show fewer" (`expanded`). A row that needs you never hides in the
 * tail; when one would, put its mark in `attention`, such as an `agent-waiting` `MetaCount`. It is
 * a button by default.
 *
 * @import import { MoreItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const MoreItem: FC<
	{
		label: string;
		expanded?: boolean;
		attention?: ReactNode;
		depth?: number;
	} & useRender.ComponentProps<"button">
> = ({ label, expanded = false, attention, depth = 0, render, ...props }) =>
	useRender({
		// oxlint-disable-next-line jsx_a11y/control-has-associated-label -- Labelled by its children.
		render: render ?? <button type="button" />,
		props: mergeProps<"button">(props, {
			"aria-expanded": expanded,
			className: classes(styles.row, styles.compact, styles.more),
			children: (
				<>
					<Steps depth={depth} folded={undefined} onToggleFolded={undefined} name={label} />
					<span className={styles.icon}>
						<Icon size={12} name={expanded ? "chevron-down" : "chevron-right"} />
					</span>
					<span className={classes("text-13", styles.label, styles.name)}>{label}</span>
					{attention}
				</>
			),
		}),
	});

/**
 * The line between the worktrees of one repository, and around a stack of branches: before it
 * unless it comes first, after it unless it ends the list.
 *
 * @import import { WorktreeDivider } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const WorktreeDivider: FC<{ depth?: number } & ComponentProps<"div">> = ({
	depth = 0,
	...props
}) => (
	// A line to group rows by eye; the tree's levels already say what belongs together.
	<div {...props} aria-hidden className={classes(props.className, styles.divider)}>
		<Steps depth={depth} folded={undefined} onToggleFolded={undefined} name="" />
		<span className={styles.line} />
	</div>
);
