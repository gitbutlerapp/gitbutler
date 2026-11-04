import { mergeProps, useRender } from "@base-ui/react";
import type { ComponentProps, FC, PointerEvent, ReactNode } from "react";
import { Badge, type BadgeVariant } from "./Badge.tsx";
import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import { MetaCount } from "./MetaCount.tsx";
import styles from "./SidebarRow.module.css";
import { StatusLed } from "./StatusLed.tsx";
import { Tooltip } from "./Tooltip.tsx";

/** What every sidebar row takes. */
type RowProps = {
	/** How many levels the row sits below the top of the card, one guide per level. */
	depth?: number;
	/** Gives the row a chevron: `true` while its children are hidden. Leave it out on a leaf.
	 * `"empty"` on a row that could hold children and has none yet, such as a branch without
	 * commits: a dot in the chevron's place keeps it in line with the rows that fold. */
	folded?: boolean | "empty";
	onToggleFolded?: () => void;
	/** Selected: the quiet fill, or the solid one while the list holds the selection focus. */
	selected?: boolean;
	/** `MetaCount`s and a `CiStatus`, in the order the row should read them. */
	meta?: ReactNode;
	/** The row's next step, as small outline `Button`s: Push, Update, Create PR, View PR. They take
	 * the state's place under the pointer and on keyboard focus; leave them out when there is none,
	 * and the row keeps its state there. */
	actions?: ReactNode;
	/** The row's menu button, a small ghost icon-only `Button`. It shows under the pointer, on
	 * keyboard focus and on a selected row. */
	menu?: ReactNode;
	/** What the name stands for in full (a path, a pull request's state, a commit's author), shown
	 * over the name alone, so the counts beside it keep their own. */
	tooltip?: ReactNode;
	/** An action on the row is running, whether started from its menu or its `actions`: a spinner
	 * stands in for its glyph until it is done. Every row shows its work there, at its start, so the
	 * state and the menu at its end keep their places. */
	busy?: boolean;
} & useRender.ComponentProps<"div">;

const Steps: FC<{
	depth: number;
	folded: boolean | "empty" | undefined;
	onToggleFolded: (() => void) | undefined;
	name: string;
}> = ({ depth, folded, onToggleFolded, name }) =>
	depth > 0 || folded !== undefined ? (
		<span className={styles.steps}>
			{Array.from({ length: depth }, (_, level) => (
				<span key={level} className={classes(styles.step, styles.guide)} aria-hidden />
			))}
			{folded === "empty" && (
				<span className={classes(styles.step, styles.empty)} aria-hidden>
					<Icon size={12} name="bullet-disc" />
				</span>
			)}
			{typeof folded === "boolean" && (
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
	const name = (
		<span
			className={className}
			data-name
			// The clip is a scroll box, which find-in-page and scrollIntoView can scroll and leave
			// scrolled, with no fade at its start. A cut name only ever runs.
			onScroll={(event) => {
				event.currentTarget.scrollLeft = 0;
			}}
		>
			<span className={styles.run}>{children}</span>
		</span>
	);
	return tooltip === undefined ? name : <Tooltip content={tooltip}>{name}</Tooltip>;
};

/** A slot the host filled with nothing (`null`, `false`) is no slot: the row keeps the room and
 * the state it would otherwise give up for it. */
const filled = (slot: ReactNode) => (slot === null || slot === false ? undefined : slot);

/** Px per second a cut name runs at under the pointer. */
const RUN_SPEED = 24;

/** Measures how far each cut name in the row has to run to show its end, at the moment the pointer
 * arrives: CSS can't size a move by how much of its own content is clipped. */
const measureRuns = (event: PointerEvent<HTMLElement>) => {
	for (const name of event.currentTarget.querySelectorAll<HTMLElement>("[data-name]")) {
		const overflow = name.scrollWidth - name.clientWidth;
		name.toggleAttribute("data-runs", overflow > 0);
		name.style.setProperty("--run", `${overflow}px`);
		name.style.setProperty("--run-duration", `${1 + overflow / RUN_SPEED}s`);
	}
};

/** The one line every sidebar item is drawn on: the name takes the line and the state sits at the
 * end, where it stays; the menu follows it under the pointer, and the actions take its place.
 * `spinner` is what stands in for `icon` while it is `busy`, a plain one unless the item colours
 * its own. A row without an `icon` starts with its name. */
const Row: FC<
	RowProps & {
		name: string;
		icon?: ReactNode;
		spinner?: ReactNode;
		label: ReactNode;
		rowClassName?: string;
	}
> = ({
	name,
	icon,
	spinner = <Icon name="spinner" />,
	label,
	busy = false,
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
			className: classes(styles.row, styles.compact, rowClassName),
			onPointerEnter: measureRuns,
			children: (
				<>
					<Steps depth={depth} folded={folded} onToggleFolded={onToggleFolded} name={name} />
					{busy ? (
						// The spinner itself is hidden from a screen reader; its cell says what it means.
						<output className={styles.icon} aria-label="Working">
							{spinner}
						</output>
					) : (
						icon !== undefined && <span className={styles.icon}>{icon}</span>
					)}
					<span className={styles.label}>{label}</span>
					{
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
					}
					{menu !== undefined && (
						<span className={styles.menu} data-menu>
							{menu}
						</span>
					)}
				</>
			),
		}),
	});
};

/**
 * A machine in the sidebar: the card's header when the sidebar is grouped by machine, a row
 * inside a repository when grouped by repository. Its `icon` is an {@link Icon} for the host, the
 * same whether it is online or not: its light says which, so the icon doesn't read as disabled.
 *
 * Folded, its `meta` sums up what it holds (`repos`, or `worktrees` when grouped by repository)
 * and its age. Open as a card's header, it shows no age: its age is the newest of the rows under
 * it, which show their own.
 *
 * @import import { MachineItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const MachineItem: FC<{ name: string; icon: ReactNode; online: boolean } & RowProps> = ({
	name,
	icon,
	online,
	tooltip,
	...props
}) => (
	<Row
		{...props}
		name={name}
		icon={<span className={styles.picture}>{icon}</span>}
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
 * {@link MachineItem} doesn't.
 *
 * `icon` defaults to the repository glyph; a host can pass the repository's own avatar.
 *
 * @import import { RepoItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const RepoItem: FC<{ name: string; icon?: ReactNode } & RowProps> = ({
	name,
	icon,
	tooltip,
	...props
}) => (
	<Row
		{...props}
		name={name}
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
 * A `fork` is the main worktree of another clone of the repository, one whose remote is a fork:
 * it takes the folder-fork glyph, leads with a "Fork" badge, as a pull request leads with its
 * number, and is named after the fork's owner rather than "Main worktree".
 *
 * @import import { WorktreeItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const WorktreeItem: FC<{ name: string; main?: boolean; fork?: boolean } & RowProps> = ({
	name,
	main = false,
	fork = false,
	tooltip,
	...props
}) => (
	<Row
		{...props}
		name={name}
		icon={<Icon name={fork ? "folder-fork" : main ? "folder" : "folder-copy"} />}
		label={
			<>
				{fork && (
					<Badge variant="lightGray" className={styles.badge}>
						Fork
					</Badge>
				)}
				<Name tooltip={tooltip} className={classes("text-14", "text-semibold", styles.name)}>
					{name}
				</Name>
			</>
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
 * Its `meta` reads in one order: `conflicts`, then `CiStatus`, ↓ `behind`, ↑ `unpushed`,
 * `uncommitted`, and `commits`. `conflicts` counts its conflicted commits while it is folded; on an
 * expanded branch drop it, since each {@link CommitItem} marks its own. Show ↑ or ⟜, never both; on an expanded branch drop ⟜, since its commits are listed
 * under it. A branch with no commits yet leads with `empty` saying "empty", and takes
 * `folded="empty"`, so its row isn't bare and lines up with the branches that fold. `actions` hold the one next step (Resolve, Update, Push, Create PR or Merge),
 * then View PR on a branch with a request.
 *
 * A merged request is done with: its title recedes. A branch merged into its target with no request
 * known takes `merged`: the same muted title, its branch glyph and a Merged badge.
 *
 * While an action on the branch runs, its `busy` spinner takes the glyph's colour. A merge takes `pr` state `merged` as it starts, so the row reads as where it is heading;
 * back to `open` if it is refused.
 *
 * @import import { BranchItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const BranchItem: FC<
	{
		title: string;
		pr?: { number: number; state: PullRequestState };
		/** Merged into the target with no pull request known. Ignored with `pr`. */
		merged?: boolean;
	} & RowProps
> = ({ title, pr, merged = false, tooltip, ...props }) => {
	const state = pr?.state ?? (merged ? "merged" : undefined);
	const glyph = (name: IconName) => (
		<Icon
			name={name}
			className={classes(styles.branchGlyph, state !== undefined && styles[state])}
		/>
	);
	const icon = glyph(pr ? prGlyph[pr.state] : "branch");
	const spinner = glyph("spinner");
	const badge = state !== undefined && (
		<Badge variant={prBadge[state]} className={styles.badge}>
			{pr ? `#${pr.number}` : "Merged"}
		</Badge>
	);

	return (
		<Row
			{...props}
			name={title}
			rowClassName={classes(state === "merged" && styles.merged)}
			icon={icon}
			spinner={spinner}
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

/**
 * A commit under an expanded branch: the hollow glyph while it is only here, the solid one once it
 * is pushed. Its `menu` shows under the pointer, on keyboard focus and when selected.
 *
 * A `conflicted` commit takes the warning in red in its glyph's place, pushed or not, so the
 * commits to resolve stand out of the list. Its branch counts them in `conflicts` while folded;
 * open, the marks on its commits say it.
 *
 * Under a {@link TargetItem}, a `merge` takes the merge glyph and, with `folded`, opens to the
 * commits it brought in, one level deeper.
 *
 * @import import { CommitItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const CommitItem: FC<
	{ message: string; pushed?: boolean; merge?: boolean; conflicted?: boolean } & RowProps
> = ({ message, pushed = true, merge = false, conflicted = false, tooltip, ...props }) => (
	<Row
		{...props}
		name={message}
		icon={
			conflicted ? (
				// oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A glyph with a name, not a picture with a source.
				<span role="img" aria-label="Conflicted" className={styles.conflicted}>
					<Icon name="warning" />
				</span>
			) : (
				<Icon name={merge ? "branch-merge" : pushed ? "commit-fill" : "commit"} />
			)
		}
		label={
			<Name tooltip={tooltip} className={classes("text-13", styles.name)}>
				{message}
			</Name>
		}
	/>
);

/**
 * What the repository's target (`target`, such as `master`) has that its workspace doesn't: the
 * first row under the repository, right over its worktrees with no {@link WorktreeDivider}
 * between. It says how many
 * commits (`count`) and keeps Update (`actions`) beside them rather than in their place under the
 * pointer, since taking them in is what the row is for. Open, the commits sit under it one level
 * deeper as {@link CommitItem}s, a merge's own one more. Leave it out while there are none.
 *
 * @import import { TargetItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const TargetItem: FC<{ target: string; count: number } & Omit<RowProps, "meta">> = ({
	target,
	count,
	actions,
	tooltip,
	...props
}) => (
	<Row
		{...props}
		name={`New on ${target}`}
		rowClassName={styles.target}
		icon={<Icon name="arrow-in-box" />}
		label={
			<Name tooltip={tooltip} className={classes("text-14", "text-semibold", styles.name)}>
				New on {target}
			</Name>
		}
		meta={
			<>
				<MetaCount type="behind">{count}</MetaCount>
				{filled(actions)}
			</>
		}
	/>
);

/**
 * A worktree's uncommitted changes, as a row of its own, with how many files it has changed
 * (`count`). It sits where the work does, first among the worktree's branches:
 * under the worktree, or under a repository with only one. It opens the changes themselves, as a
 * branch opens its commits; the worktree's own row opens its overview. Leave it out while the
 * worktree is clean. Its `menu` shows under the pointer, on keyboard focus and when selected.
 *
 * @import import { UncommittedItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const UncommittedItem: FC<
	{ count: number } & Omit<RowProps, "folded" | "onToggleFolded" | "meta" | "actions">
> = ({ count, tooltip, ...props }) => (
	<Row
		{...props}
		name="Uncommitted changes"
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
 * The caption over a stack of two or more branches: how many (`count`) and "Stack". Its branches
 * sit under it one level deeper, with no divider around the stack. It doesn't open; with `folded` a
 * click anywhere on it, as on its chevron, hides the stack's branches. Folded, the branch at the
 * stack's top (`tip`), the one that gets pushed, follows "Stack", muted and giving way first, and
 * `meta` says what the hidden branches hold. "Stack" keeps one quiet colour either way, so the
 * caption doesn't read as one more branch. Its `menu` (push the stack, open its pull requests)
 * shows under the pointer.
 *
 * @import import { StackCaption } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const StackCaption: FC<
	{ count: number; tip: string } & Pick<
		RowProps,
		"depth" | "folded" | "onToggleFolded" | "meta" | "menu" | "tooltip" | "busy"
	> &
		ComponentProps<"div">
> = ({ count, tip, depth = 0, folded, onToggleFolded, meta, menu, tooltip, busy, ...props }) => {
	const folds = typeof folded === "boolean" && onToggleFolded !== undefined;
	return (
		<Row
			{...props}
			name={`Stack of ${count}`}
			depth={depth}
			folded={folded}
			onToggleFolded={onToggleFolded}
			meta={meta}
			menu={menu}
			busy={busy}
			onClick={(event) => {
				props.onClick?.(event);
				const target = event.target as Element;
				// Not from the menu: its button sits in the row, and its items bubble here through their
				// portal while being no part of the row on the page.
				if (
					!folds ||
					event.defaultPrevented ||
					!event.currentTarget.contains(target) ||
					target.closest("[data-menu]")
				)
					return;
				onToggleFolded();
			}}
			rowClassName={classes(styles.caption, folds && styles.captionFolds)}
			icon={<Icon name="stack" />}
			label={
				<>
					<Badge variant="lightGray" className={styles.badge}>
						{count}
					</Badge>
					<Name
						tooltip={tooltip}
						className={classes("text-14", "text-semibold", styles.name, styles.word)}
					>
						Stack
					</Name>
					{folded === true && (
						<Name tooltip={tooltip} className={classes("text-14", "text-semibold", styles.name)}>
							{tip}
						</Name>
					)}
				</>
			}
		/>
	);
};

/**
 * The last row of a long list, which shows twelve at a time: "12 more" opens the next page, and
 * once everything shows it reads "Show fewer" (`expanded`). The selected row never hides in the
 * tail. It is a button by default.
 *
 * @import import { MoreItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const MoreItem: FC<
	{
		label: string;
		expanded?: boolean;
		depth?: number;
	} & useRender.ComponentProps<"button">
> = ({ label, expanded = false, depth = 0, render, ...props }) =>
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
						<Icon size={12} name={expanded ? "chevron-up" : "kebab"} />
					</span>
					<span className={classes("text-13", styles.label, styles.name)}>{label}</span>
				</>
			),
		}),
	});

/**
 * The label over a group of pages in a navigation panel, such as the docs site's "Getting
 * started": its name, small and quiet, so the pages under it read first. With `folded` it is a
 * button that hides and shows them, its chevron saying which; without, it only names them. Its
 * pages sit one level under it, as {@link NavItem}s at `depth={1}`. Like every sidebar row, the
 * label and its pages sit in a card (`--bg-1` in a `--border-section` ring at `--radius-card`),
 * never loose on a panel's ground.
 *
 * @import import { GroupLabel } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const GroupLabel: FC<
	{
		name: string;
		folded?: boolean;
		onToggleFolded?: () => void;
	} & useRender.ComponentProps<"button">
> = ({ name, folded, onToggleFolded, render, ...props }) =>
	useRender({
		// oxlint-disable-next-line jsx_a11y/control-has-associated-label -- Labelled by its children.
		render: render ?? (folded === undefined ? <div /> : <button type="button" />),
		props: mergeProps<"button">(props, {
			"aria-expanded": folded === undefined ? undefined : !folded,
			onClick: onToggleFolded,
			className: classes(styles.row, styles.compact, styles.groupLabel),
			children: (
				<>
					{folded !== undefined && (
						<span className={styles.step}>
							<Icon size={12} name={folded ? "chevron-right" : "chevron-down"} />
						</span>
					)}
					<span className={classes("text-12", "text-semibold", styles.label, styles.name)}>
						{name}
					</span>
				</>
			),
		}),
	});

/**
 * A page in a navigation panel, such as the docs site's: its name, and the glyph that names the
 * page when there is one (`icon`). It sits one level under its {@link GroupLabel} (`depth={1}`), on
 * the guide that ties it to the label's chevron, the way a sidebar row sits under the row that folds
 * it. It opens a page rather than folding, so it has no chevron and no menu: render it as the link
 * that opens the page (`render={<a href={…} />}`). `selected` is the page on screen, which the link
 * also marks with `aria-current="page"`. It sits in a card with its {@link GroupLabel}, never loose
 * on a panel's ground.
 *
 * @import import { NavItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const NavItem: FC<
	{ name: string; icon?: IconName } & Omit<
		RowProps,
		"folded" | "onToggleFolded" | "meta" | "actions" | "menu" | "busy"
	>
> = ({ name, icon, tooltip, ...props }) => (
	<Row
		{...props}
		name={name}
		icon={icon === undefined ? undefined : <Icon name={icon} />}
		label={
			<Name tooltip={tooltip} className={classes("text-13", styles.name)}>
				{name}
			</Name>
		}
	/>
);

/**
 * Where rows should be and can't show, in their place: what failed (`message`, one sentence) under
 * the warning glyph, in grey, and the way out (`action`, such as Try again) as a link after it. The
 * reason, when there is one beyond the message, goes in `tooltip`. Rows that load from somewhere
 * the card's own data doesn't, such as a {@link TargetItem}'s commits, use it rather than open onto
 * nothing.
 *
 * @import import { NoticeItem } from "@gitbutler/ui-react/SidebarRow.tsx";
 */
export const NoticeItem: FC<
	{ message: string; action?: { label: string; onClick: () => void } } & Omit<
		RowProps,
		"folded" | "onToggleFolded" | "meta" | "actions" | "menu" | "busy"
	>
> = ({ message, action, tooltip, ...props }) => (
	<Row
		{...props}
		name={message}
		icon={<Icon name="warning" />}
		label={
			<Name tooltip={tooltip} className={classes("text-13", styles.name, styles.notice)}>
				{message}
				{action && (
					<>
						{" "}
						<button type="button" className={styles.noticeAction} onClick={action.onClick}>
							{action.label}
						</button>
					</>
				)}
			</Name>
		}
	/>
);

/**
 * The line between the worktrees of one repository. A stack needs none: its branches sit one level
 * under its {@link StackCaption}.
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
