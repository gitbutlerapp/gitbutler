import { mergeProps, useRender } from "@base-ui/react";
import type { ComponentProps, FC, PointerEvent, ReactElement, ReactNode } from "react";
import { Avatar } from "./Avatar.tsx";
import { Badge, type BadgeVariant } from "./Badge.tsx";
import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import { MachineStatus } from "./MachineStatus.tsx";
import type { IconName } from "./iconNames.ts";
import { MetaCount } from "./MetaCount.tsx";
import styles from "./SidebarRow.module.css";
import { Tooltip } from "./Tooltip.tsx";
import type { Guides } from "./treeGuides.ts";

/** What every sidebar row takes. */
type RowProps = {
	/** How many levels the row sits below the top of the card, one guide per level. */
	depth?: number;
	/** How each level's guide draws on this row, outermost first: work them out with
	 * `treeGuides` (treeGuides.ts) from the rows the tree shows, since a row can't see the rows after it. */
	guides?: Guides;
	/** Gives the row a chevron, in a step of its own before its glyph: `true` while its children
	 * are hidden. Leave it out on a leaf. `"empty"` on a row that could hold children and has none
	 * yet, such as a branch without commits: a dot in the chevron's place keeps it in line with the
	 * rows that fold. */
	folded?: boolean | "empty";
	onToggleFolded?: () => void;
	/** Selected: the quiet fill, or the solid one while the list holds the selection focus. */
	selected?: boolean;
	/** `MetaCount`s and a `CiStatus`, in the order the row should read them. */
	meta?: ReactNode;
	/** The row's next step, a small outline `Button`, after its state and always shown. A step that
	 * acts on a count takes that count's place: leave the count out of `meta` and give the button its
	 * number, then its glyph, in its colour (`info` Push for ↑ `unpushed`, `warning` Update for ↓
	 * `behind`). The rest of what the row can do is in its menu, which opens with a right-click, or
	 * Shift-F10 on the focused row: make the row a `ContextMenu`'s `trigger`. It has no ⋯. */
	actions?: ReactNode;
	/** What the name stands for in full (a path, a pull request's state, a commit's author), shown
	 * over the name alone, so the counts beside it keep their own. */
	tooltip?: ReactNode;
	/** An action on the row is running, whether started from its menu or its `actions`: a spinner
	 * stands in for its glyph until it is done. Every row shows its work there, at its start, so the
	 * state at its end keeps its place. */
	busy?: boolean;
	/** Something on the row changed since it was last opened: a dot at the row's very end, after its
	 * state. Give every row in the tree a value, `false` too, once the host tracks it: a row with one
	 * keeps the dot's room whether it shows or not, so the counts line up down the list. */
	unread?: boolean;
} & useRender.ComponentProps<"div">;

const Steps: FC<{
	depth: number;
	guides?: Guides;
	folded: boolean | "empty" | undefined;
	onToggleFolded: (() => void) | undefined;
	name: string;
	/** The row itself is the button that folds, so the chevron only says which way it is. */
	rowFolds?: boolean;
}> = ({ depth, guides, folded, onToggleFolded, name, rowFolds = false }) =>
	depth > 0 || folded !== undefined ? (
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
			{folded === "empty" && (
				<span className={classes(styles.step, styles.empty)} aria-hidden>
					<Icon size={12} name="bullet-disc" />
				</span>
			)}
			{typeof folded === "boolean" && rowFolds && (
				<span className={classes(styles.step, styles.toggle)} aria-hidden>
					<Icon size={12} name={folded ? "chevron-right" : "chevron-down"} />
				</span>
			)}
			{typeof folded === "boolean" && !rowFolds && (
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

/** The one line every sidebar item is drawn on: the name takes the line, and the state and then the
 * next step sit at the end, where they stay.
 * `spinner` is what stands in for `icon` while it is `busy`, a plain one unless the item colours
 * its own. A row without an `icon` starts with its name, after its chevron if it folds. */
const Row: FC<
	RowProps & {
		name: string;
		icon?: ReactNode;
		spinner?: ReactNode;
		label: ReactNode;
		rowClassName?: string;
		rowFolds?: boolean;
	}
> = ({
	name,
	icon,
	rowFolds,
	spinner = <Icon name="spinner" />,
	label,
	busy = false,
	unread,
	depth = 0,
	guides,
	folded,
	onToggleFolded,
	selected = false,
	meta: metaSlot,
	actions: actionsSlot,
	rowClassName,
	render,
	...props
}) => {
	const [meta, actions] = [filled(metaSlot), filled(actionsSlot)];
	return useRender({
		render: render ?? <div />,
		state: { selected },
		props: mergeProps<"div">(props, {
			className: classes(styles.row, styles.compact, rowClassName),
			onPointerEnter: measureRuns,
			children: (
				<>
					<Steps
						depth={depth}
						guides={guides}
						folded={folded}
						onToggleFolded={onToggleFolded}
						name={name}
						rowFolds={rowFolds}
					/>
					{busy ? (
						// The spinner itself is hidden from a screen reader; its cell says what it means.
						<output className={styles.icon} aria-label="Working">
							{spinner}
						</output>
					) : (
						icon !== undefined && <span className={styles.icon}>{icon}</span>
					)}
					<span className={styles.label}>{label}</span>
					{(meta !== undefined || actions !== undefined) && (
						<span className={styles.end}>
							{meta !== undefined && <span className={styles.meta}>{meta}</span>}
							{actions !== undefined && (
								<span className={styles.actions} data-actions>
									{actions}
								</span>
							)}
						</span>
					)}
					{unread === true && (
						// oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A drawn dot with a name, not a picture with a source.
						<span role="img" aria-label="Unread" className={styles.unread} />
					)}
					{unread === false && <span aria-hidden className={classes(styles.unread, styles.read)} />}
				</>
			),
		}),
	});
};

/**
 * A machine in the sidebar: the card's header when the sidebar is grouped by machine, a row
 * inside a repository when grouped by repository. Its `icon` is an {@link Icon} for the host, the
 * same whether it is online or not: its {@link MachineStatus} says which, so the icon doesn't read
 * as disabled.
 *
 * Its status says "Online" or "Offline" under the pointer; `statusTooltip` says more in its
 * place, such as how long the machine has run or when it was last seen.
 *
 * Folded, its `meta` sums up what it holds (`repos`, or `worktrees` when grouped by repository)
 * and its age. Open as a card's header, it shows no age: its age is the newest of the rows under
 * it, which show their own.
 *
 * @import import { MachineItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
 */
export const MachineItem: FC<
	{ name: string; icon: ReactNode; online: boolean; statusTooltip?: ReactNode } & RowProps
> = ({ name, icon, online, statusTooltip, tooltip, ...props }) => (
	<Row
		{...props}
		name={name}
		icon={<span className={styles.picture}>{icon}</span>}
		label={
			<>
				<Name tooltip={tooltip} className={classes("text-14", "text-semibold", styles.name)}>
					{name}
				</Name>
				<Tooltip content={statusTooltip ?? (online ? "Online" : "Offline")}>
					<MachineStatus online={online} />
				</Tooltip>
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
 * @import import { RepoItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
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
 * @see packages/but-ui/design/components/sidebar-trees.md
 * @import import { WorktreeItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
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
 * always shown after the counts: Update and Push take the place of ↓ and ↑, as their number and
 * arrow in a `warning` or `info` button. Opening the pull request is in the row's menu.
 *
 * A merged request is done with: its title recedes. A branch merged into its target with no request
 * known takes `merged`: the same muted title, its branch glyph and a Merged badge.
 *
 * While an action on the branch runs, its `busy` spinner takes the glyph's colour. A merge takes `pr` state `merged` as it starts, so the row reads as where it is heading;
 * back to `open` if it is refused.
 *
 *
 * @figma `pr` follows the pull request: `none` shows the branch's name, the others its number and
 * title, and `none-merged` is `merged` with no request known. The counts come from BranchMeta: on
 * an expanded branch turn off commits (⟜), and turn on conflicts only while it's folded. For a
 * branch with no commits yet, turn on commits alone and set its text to empty. For Push, turn on
 * BranchMeta's action and turn unpushed off. `unread` draws the
 * dot after the counts, and `read` keeps its room, as `unread={false}` does, so the counts line up
 * down the list; every tree row has both.
 * @see packages/but-ui/design/components/sidebar-trees.md
 * @import import { BranchItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
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
 * A commit under an expanded branch: the hollow glyph in blue while it is only here, work to send
 * as ↑ `unpushed` is, and the solid one, muted, once it is pushed.
 *
 * A `conflicted` commit takes the warning in amber in its glyph's place, pushed or not, so the
 * commits to resolve stand out of the list. Its branch counts them in `conflicts` while folded;
 * open, the marks on its commits say it.
 *
 * Under a {@link TargetItem}, a `merge` takes the merge glyph and, with `folded`, opens to the
 * commits it brought in, one level deeper. There each commit comes from someone else, so give its
 * `author`: their {@link Avatar} sits at the row's end, after any `meta`, where a count would, so
 * the glyphs down the list keep saying what each row is. A merge has no author of its own; its
 * commits carry the faces. Name the author in the `tooltip`; the picture is decorative.
 *
 * @import import { CommitItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
 */
export const CommitItem: FC<
	{
		message: string;
		pushed?: boolean;
		merge?: boolean;
		conflicted?: boolean;
		author?: CommitAuthor;
	} & RowProps
> = ({
	message,
	pushed = true,
	merge = false,
	conflicted = false,
	author,
	tooltip,
	meta,
	...props
}) => (
	<Row
		{...props}
		name={message}
		meta={
			author && !merge ? (
				<>
					{meta}
					<span className={styles.author}>
						<Avatar src={author.src} seed={author.seed} size={14} />
					</span>
				</>
			) : (
				meta
			)
		}
		icon={
			conflicted ? (
				// oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A glyph with a name, not a picture with a source.
				<span role="img" aria-label="Conflicted" className={styles.conflicted}>
					<Icon name="warning" />
				</span>
			) : (
				<Icon
					name={merge ? "branch-merge" : pushed ? "commit-fill" : "commit"}
					className={classes(!merge && !pushed && styles.unpushed)}
				/>
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
 * first row under the repository, right over its worktrees. It says how many
 * commits (`count`), and Update (`actions`) takes that count's place, as on every row. Open, the commits sit under it one level
 * deeper as {@link CommitItem}s, a merge's own one more. Leave it out while there are none.
 *
 * `authors`, the people the commits come from, each once, show as overlapping pictures before the
 * count, so who is coming in reads with the list folded. It draws the first three; name them all
 * in the `tooltip`.
 *
 * @import import { TargetItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
 */
export const TargetItem: FC<
	{ target: string; count: number; authors?: ReadonlyArray<CommitAuthor> } & Omit<RowProps, "meta">
> = ({ target, count, authors = [], actions, tooltip, ...props }) => (
	<Row
		{...props}
		name={`New on ${target}`}
		icon={<Icon name="arrow-in-box" />}
		label={
			<Name tooltip={tooltip} className={classes("text-14", "text-semibold", styles.name)}>
				New on {target}
			</Name>
		}
		meta={
			<>
				{authors.length > 0 && (
					<span className={styles.authors}>
						{authors.slice(0, 3).map((a) => (
							<Avatar key={a.seed} src={a.src} seed={a.seed} size={14} />
						))}
					</span>
				)}
				{filled(actions) === undefined && <MetaCount type="behind">{count}</MetaCount>}
			</>
		}
		actions={actions}
	/>
);

/** Who a commit is from: their picture's `src` if the host has one, and `seed`, their email, which
 * finds their Gravatar and picks their colour without one. */
export type CommitAuthor = { src?: string | null; seed: string };

/**
 * A worktree's uncommitted changes, as a row of its own, with how many files it has changed
 * (`count`). It sits where the work does, first among the worktree's branches:
 * under the worktree, or under a repository with only one. It opens the changes themselves, as a
 * branch opens its commits; the worktree's own row opens its overview. Leave it out while the
 * worktree is clean.
 *
 * @import import { UncommittedItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
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
 * caption doesn't read as one more branch. `actions` is the stack's next step, as on any row: a
 * Push for the whole stack in place of its ↑. The rest (open its pull requests) is in its menu,
 * which opens with a right-click, as every row's does.
 *
 * @see packages/but-ui/design/components/sidebar-trees.md
 * @import import { StackCaption } from "@gitbutler/but-ui-react/SidebarRow.tsx";
 */
export const StackCaption: FC<
	{ count: number; tip: string } & Pick<
		RowProps,
		| "depth"
		| "guides"
		| "folded"
		| "onToggleFolded"
		| "meta"
		| "actions"
		| "tooltip"
		| "busy"
		| "unread"
	> &
		ComponentProps<"div">
> = ({
	count,
	tip,
	depth = 0,
	folded,
	onToggleFolded,
	meta,
	actions,
	tooltip,
	busy,
	unread,
	...props
}) => {
	const folds = typeof folded === "boolean" && onToggleFolded !== undefined;
	return (
		<Row
			{...props}
			name={`Stack of ${count}`}
			depth={depth}
			folded={folded}
			onToggleFolded={onToggleFolded}
			meta={meta}
			actions={actions}
			busy={busy}
			unread={unread}
			onClick={(event) => {
				props.onClick?.(event);
				const target = event.target as Element;
				// Not from a popup's items, which bubble here through their portal while being no part of
				// the row on the page, nor from its next step, which does its own thing.
				if (
					!folds ||
					event.defaultPrevented ||
					!event.currentTarget.contains(target) ||
					target.closest("[data-actions]") !== null
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
 * @import import { MoreItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
 */
export const MoreItem: FC<
	{
		label: string;
		expanded?: boolean;
		depth?: number;
		guides?: Guides;
	} & useRender.ComponentProps<"button">
> = ({ label, expanded = false, depth = 0, guides, render, ...props }) =>
	useRender({
		// oxlint-disable-next-line jsx_a11y/control-has-associated-label -- Labelled by its children.
		render: render ?? <button type="button" />,
		props: mergeProps<"button">(props, {
			"aria-expanded": expanded,
			className: classes(styles.row, styles.compact, styles.more),
			children: (
				<>
					<Steps
						depth={depth}
						guides={guides}
						folded={undefined}
						onToggleFolded={undefined}
						name={label}
					/>
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
 * label and its pages sit in a card (`--but-background` in a `--but-border-subtle` ring at `--but-radius-card`),
 * never loose on a panel's ground.
 *
 * @import import { GroupLabel } from "@gitbutler/but-ui-react/SidebarRow.tsx";
 */
export const GroupLabel: FC<
	{
		name: string;
		/** The group's glyph, an icon's name or an element of its own, after its chevron. */
		icon?: IconName | ReactElement;
		folded?: boolean;
		onToggleFolded?: () => void;
	} & useRender.ComponentProps<"button">
> = ({ name, icon, folded, onToggleFolded, render, ...props }) =>
	useRender({
		// oxlint-disable-next-line jsx_a11y/control-has-associated-label -- Labelled by its children.
		render: render ?? (folded === undefined ? <div /> : <button type="button" />),
		props: mergeProps<"button">(props, {
			"aria-expanded": folded === undefined ? undefined : !folded,
			onClick: onToggleFolded,
			className: classes(styles.row, styles.compact, styles.groupLabel),
			children: (
				<>
					<Steps depth={0} folded={folded} onToggleFolded={undefined} name={name} rowFolds />
					{icon !== undefined && (
						<span className={styles.icon}>
							{typeof icon === "string" ? <Icon name={icon} /> : icon}
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
 * page when there is one (`icon`, an icon's name, or an element of its own such as a colour's
 * swatch or an {@link Avatar}, drawn at 14px). Its `meta` sits at the row's end, as on every
 * sidebar row: plain text there reads as the row's current value ("Light", "Teal"), in 13 and
 * subtle, and a `MetaCount` or a `Badge` keeps its own look. It sits one level under its {@link GroupLabel} (`depth={1}`), on
 * the guide that ties it to the label's chevron, the way a sidebar row sits under the row that folds
 * it. It opens a page rather than folding, so it has no chevron and no menu: render it as the link
 * that opens the page (`render={<a href={…} />}`). `selected` is the page on screen, which the link
 * also marks with `aria-current="page"`. It sits in a card with its {@link GroupLabel}, never loose
 * on a panel's ground.
 *
 * `leadingRow` is the row that heads a card of groups, such as the docs site's Docs and Components:
 * its name a step louder, in 14 semibold, and a chevron (`folded`, `onToggleFolded`) that folds the
 * whole card away. It sits alone above the card's first group, past a hairline. Rendered as a link
 * it opens the card's own page, as Docs opens the overview, and only its chevron folds; with no page
 * of its own, the whole row is the button that folds.
 *
 * @figma `selected` is the fill a focused list shows; `selected-inactive` the quiet one, as on
 * every sidebar row.
 * @import import { NavItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
 */
export const NavItem: FC<
	{
		name: string;
		icon?: IconName | ReactElement;
		leadingRow?: boolean;
		/** A leading row's chevron: `true` while its card's groups are hidden. */
		folded?: boolean;
		onToggleFolded?: () => void;
	} & Omit<RowProps, "folded" | "onToggleFolded" | "actions" | "busy">
> = ({
	name,
	icon,
	leadingRow = false,
	folded,
	onToggleFolded,
	meta,
	tooltip,
	render,
	...props
}) => {
	const folds = leadingRow && folded !== undefined;
	const rowFolds = folds && render === undefined;
	return (
		<Row
			{...props}
			name={name}
			folded={folds ? folded : undefined}
			onToggleFolded={folds && !rowFolds ? onToggleFolded : undefined}
			rowFolds={rowFolds}
			render={
				rowFolds ? (
					// oxlint-disable-next-line jsx_a11y/control-has-associated-label -- Labelled by its children.
					<button type="button" aria-expanded={!folded} onClick={onToggleFolded} />
				) : (
					render
				)
			}
			rowClassName={styles.nav}
			icon={typeof icon === "string" ? <Icon name={icon} /> : icon}
			meta={meta}
			label={
				<Name
					tooltip={tooltip}
					className={classes(leadingRow ? "text-14 text-semibold" : "text-13", styles.name)}
				>
					{name}
				</Name>
			}
		/>
	);
};

/**
 * Where rows should be and can't show, in their place: what failed (`message`, one sentence) under
 * the warning glyph, in grey, and the way out (`action`, such as Try again) as a link after it. The
 * reason, when there is one beyond the message, goes in `tooltip`. Rows that load from somewhere
 * the card's own data doesn't, such as a {@link TargetItem}'s commits, use it rather than open onto
 * nothing.
 *
 * @import import { NoticeItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
 */
export const NoticeItem: FC<
	{ message: string; action?: { label: string; onClick: () => void } } & Omit<
		RowProps,
		"folded" | "onToggleFolded" | "meta" | "actions" | "busy"
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
