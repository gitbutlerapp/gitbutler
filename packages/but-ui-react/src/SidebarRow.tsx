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
import { SkeletonBone } from "./SkeletonBone.tsx";
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
	/** `MetaCount`s and a `MetaStatus`, in the order the row should read them. */
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
	/** Something on the row changed since it was last opened: a dot in the row's start padding,
	 * before its guides, where it moves nothing, so the counts line up down the list either way. */
	unread?: boolean;
	/** The row is part of something the pointer is on elsewhere in the tree, as a row whose tile is
	 * in the split whose map is pointed at: it takes the hover's ground, so the rows that go together
	 * read as one while it lasts. */
	linked?: boolean;
	/** The row's `SplitPreviewMap`, while it is in a split: in a cell of its own at the row's very
	 * end, after its counts and next step, so the maps line up down the list. Give the tree
	 * `data-split-room` while any of its rows has one, and the rows without keep the cell's room, so
	 * their counts line up too. */
	split?: ReactNode;
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
	linked = false,
	meta: metaSlot,
	actions: actionsSlot,
	split: splitSlot,
	rowClassName,
	render,
	...props
}) => {
	const [meta, actions, split] = [filled(metaSlot), filled(actionsSlot), filled(splitSlot)];
	return useRender({
		render: render ?? <div />,
		state: { selected, linked },
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
					{(meta !== undefined || actions !== undefined || split !== undefined) && (
						<span className={styles.end}>
							{meta !== undefined && <span className={styles.meta}>{meta}</span>}
							{actions !== undefined && (
								<span className={styles.actions} data-actions>
									{actions}
								</span>
							)}
							{split !== undefined && <span className={styles.split}>{split}</span>}
						</span>
					)}
					{unread === true && (
						// oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A drawn dot with a name, not a picture with a source.
						<span role="img" aria-label="Unread" className={styles.unread} />
					)}
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
				<Name tooltip={tooltip} className={classes("text-14", styles.name)}>
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
			<Name tooltip={tooltip} className={classes("text-14", styles.name)}>
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
				<Name tooltip={tooltip} className={classes("text-14", styles.name)}>
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

/** Who a commit is from: their picture's `src` if the host has one, and `seed`, their email, which
 * finds their Gravatar and picks their colour without one. */
export type CommitAuthor = { src?: string | null; seed: string };

/** Who new commits come from, each once, as overlapping pictures: the first three. */
const Authors: FC<{ authors: ReadonlyArray<CommitAuthor> }> = ({ authors }) =>
	authors.length > 0 && (
		<span className={styles.authors}>
			{authors.slice(0, 3).map((a) => (
				<Avatar key={a.seed} src={a.src} seed={a.seed} size={14} />
			))}
		</span>
	);

/**
 * A branch in the sidebar. With a pull request (`pr`), the row is the request: its state in the
 * glyph and the badge, its number, and its title as `title`; without one, `title` is the branch's
 * name.
 *
 * Its `meta` reads in one order: `conflicts`, then `MetaStatus`, ↓ `behind`, ↑ `unpushed`,
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
 * dot in the row's start padding, before its guides, where it moves nothing; every tree row has
 * it.
 * @see packages/but-ui/design/components/sidebar-trees.md
 * @see packages/but-ui/design/components/sidebar-rows.md
 * @import import { BranchItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
 */
export const BranchItem: FC<
	{
		title: string;
		pr?: { number: number; state: PullRequestState };
		/** Merged into the target with no pull request known. Ignored with `pr`. */
		merged?: boolean;
		/** Who the commits on its remote that it doesn't have yet come from, shown while it's folded
		 * after its counts, as its {@link UpstreamItem} shows them while that is. */
		authors?: ReadonlyArray<CommitAuthor>;
	} & RowProps
> = ({ title, pr, merged = false, authors = [], tooltip, meta, ...props }) => {
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
			meta={
				props.folded === true && authors.length > 0 ? (
					<>
						{meta}
						<Authors authors={authors} />
					</>
				) : (
					meta
				)
			}
			label={
				<>
					{badge}
					<Name tooltip={tooltip} className={classes("text-14", styles.name, styles.title)}>
						{title}
					</Name>
				</>
			}
		/>
	);
};

/**
 * A commit under an expanded branch: the hollow glyph while it is only here, the solid one once it
 * is pushed. The glyph takes its state's `--but-commit-*` token, never a colour of its own: blue
 * `commit-local` while only here, muted `commit-remote` once pushed, amber `commit-upstream` for
 * an `upstream` commit, one on its branch's remote the workspace doesn't have yet, and purple
 * `commit-integrated` for an `integrated` one, in the target already, as a merged branch's are. A
 * state's colour changes in that token, for every client at once (but-ui's Sidebar trees note).
 *
 * A `conflicted` commit takes the warning in amber in its glyph's place, pushed or not, so the
 * commits to resolve stand out of the list. Its branch counts them in `conflicts` while folded;
 * open, the marks on its commits say it.
 *
 * Among `upstream` commits a `merge` takes the merge glyph and, with `folded`, opens to the commits
 * it brought in, one level deeper. Someone else pushed them, so give each its
 * `author`: their {@link Avatar} sits at the row's end, after any `meta`, where a count would, so
 * the glyphs down the list keep saying what each row is. A merge has no author of its own: folded,
 * it shows its commits' `authors` stacked, and open, its commits carry the faces. Name the author in the `tooltip`; the picture is decorative.
 *
 * @import import { CommitItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
 */
export const CommitItem: FC<
	{
		message: string;
		pushed?: boolean;
		upstream?: boolean;
		/** In the target already, as a merged branch's commits are: pushed, so the solid glyph, in
		 * `--but-commit-integrated`. */
		integrated?: boolean;
		merge?: boolean;
		conflicted?: boolean;
		author?: CommitAuthor;
		/** A merge's: who the commits it brought in come from, shown while it's folded. */
		authors?: ReadonlyArray<CommitAuthor>;
	} & RowProps
> = ({
	message,
	pushed = true,
	upstream = false,
	integrated = false,
	merge = false,
	conflicted = false,
	author,
	authors = [],
	tooltip,
	meta,
	...props
}) => (
	<Row
		{...props}
		name={message}
		meta={
			merge ? (
				props.folded === true && authors.length > 0 ? (
					<>
						{meta}
						<Authors authors={authors} />
					</>
				) : (
					meta
				)
			) : author ? (
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
					name={
						merge ? "branch-merge" : pushed || upstream || integrated ? "commit-fill" : "commit"
					}
					className={
						upstream
							? styles.upstream
							: merge
								? undefined
								: integrated
									? styles.integrated
									: pushed
										? styles.remote
										: styles.local
					}
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
 * What a branch's remote (`remote`, such as `origin`) has that the branch doesn't: a heading one
 * level under the branch, right over the branch's own commits. Folded, it says who the new
 * commits come from (`authors`, each once, as overlapping pictures; it draws the first three, so
 * name them all in the `tooltip`); open, it leaves that to the commits, each with its own `author`,
 * as a folded row sums up what it hides. It always says how many (`count`), and Update (`actions`)
 * takes that count's place, as on every row. Its glyph takes `--but-commit-upstream`, as the commits under it do, so the two change
 * together. Open, the commits sit under it one level deeper as `upstream` {@link CommitItem}s.
 * Leave it out while there are none.
 *
 * Name the remote alone, `origin`, and the whole ref, `origin/perf-cheaper-status-polling`, in
 * the `tooltip`: the row reads "New on origin" in a narrow sidebar.
 *
 * @import import { UpstreamItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
 */
export const UpstreamItem: FC<
	{ remote: string; count: number; authors?: ReadonlyArray<CommitAuthor> } & Omit<RowProps, "meta">
> = ({ remote, count, authors = [], actions, tooltip, folded, ...props }) => (
	<Row
		{...props}
		folded={folded}
		name={`New on ${remote}`}
		icon={<Icon name="arrow-in-box" className={styles.upstream} />}
		label={
			<Name tooltip={tooltip} className={classes("text-14", styles.name)}>
				New on {remote}
			</Name>
		}
		meta={
			<>
				{folded !== false && <Authors authors={authors} />}
				{filled(actions) === undefined && <MetaCount type="behind">{count}</MetaCount>}
			</>
		}
		actions={actions}
	/>
);

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
			<Name tooltip={tooltip} className={classes("text-14", styles.name)}>
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
					<Name tooltip={tooltip} className={classes("text-14", styles.name, styles.word)}>
						Stack
					</Name>
					{folded === true && (
						<Name tooltip={tooltip} className={classes("text-14", styles.name)}>
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
 * A terminal open on a worktree, by its `name`: the one the user gave it, else what its program
 * titled it, else the program it was started to run. Where it is open follows the name, quieter
 * and giving way first (`place`: the worktree, or the repository and the worktree), unless the
 * rows around it already say.
 *
 * What its program is at goes in `meta`. An agent's is a {@link MetaCount}'s robot alone, with no
 * value: `agent-working`, which moves, or `agent-waiting` while it waits for you.
 * Any other program that reports shows the terminal's own dots spinner while it shows progress, a
 * MetaCount `running` with no value, then a {@link MetaStatus} glyph alone: `passed` once it
 * finished, `failed` once it stopped on an error. An idle terminal shows
 * none, and nor does one running a program that reports nothing, so the ones that want something
 * stand out down the list.
 *
 * It takes no `unread` dot: its status is what you haven't seen, and the host drops `passed` and
 * `failed` once the terminal is opened. While a folded row hides it, an unseen `passed` or `failed`
 * puts the dot on that row instead, and a waiting agent takes its age's place.
 *
 * @figma `place` turns the place off and `place text` is its words, after their slash. `meta` turns
 * the status off for an idle terminal; its instance is a MetaCount robot with `value` off, swapped
 * for `running` for another program's progress, or a MetaStatus glyph once it ends.
 * @see packages/but-ui/design/components/sidebar-rows.md
 * @see packages/but-ui/design/patterns/work-under-way.md
 * @import import { TerminalItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
 */
export const TerminalItem: FC<{ name: string; place?: string } & RowProps> = ({
	name,
	place,
	tooltip,
	...props
}) => (
	<Row
		{...props}
		name={name}
		icon={<Icon name="terminal" />}
		label={
			<>
				<Name tooltip={tooltip} className={classes("text-14", styles.name)}>
					{name}
				</Name>
				{place !== undefined && (
					<span className={classes("text-13", styles.name, styles.place)}>/ {place}</span>
				)}
			</>
		}
	/>
);

/**
 * A drawer at the foot of the sidebar, such as the hidden projects or the terminals: its name, how
 * many it holds (`count`, shown at zero too) and a chevron after them. A press anywhere on it opens
 * or closes it (`folded`, `onToggleFolded`), and what it holds opens under it.
 *
 * Its buttons sit at its end and take their own clicks: `actions` always show, `hoverActions` only
 * while the pointer is on the drawer or the keys are in it, before the ones that always show, so
 * those keep their place. A ghost regular icon-only `Button` each, such as Add project or Sort and
 * show.
 *
 * Unlike the other sidebar rows it sits on the panel's ground, under the cards rather than in
 * one, and its name stays muted so the cards above read first. It is the row of a {@link Drawer},
 * which rules it off and opens what it holds under it.
 *
 * @figma `state=hover` is the drawer under the pointer: its chevron comes up and `hoverActions`
 * shows. Hide a slot the drawer doesn't need, as an empty one still keeps a button's width.
 * @import import { DrawerItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
 */
export const DrawerItem: FC<
	{
		name: string;
		count?: number;
		/** `true` while what it holds is hidden. */
		folded: boolean;
		onToggleFolded?: () => void;
		actions?: ReactNode;
		hoverActions?: ReactNode;
	} & useRender.ComponentProps<"button">
> = ({ name, count, folded, onToggleFolded, actions, hoverActions, render, ...props }) => {
	const [shown, onHover] = [filled(actions), filled(hoverActions)];
	const toggle = useRender({
		// oxlint-disable-next-line jsx_a11y/control-has-associated-label -- Labelled by its children.
		render: render ?? <button type="button" />,
		props: mergeProps<"button">(props, {
			"aria-expanded": !folded,
			onClick: onToggleFolded,
			className: styles.drawerToggle,
			children: (
				<span className={styles.label}>
					<span className={classes("text-12", "text-semibold", styles.name)}>{name}</span>
					{count !== undefined && (
						<Badge variant="lightGray" className={styles.badge}>
							{count}
						</Badge>
					)}
					<span className={styles.toggle} aria-hidden>
						<Icon size={12} name={folded ? "chevron-right" : "chevron-down"} />
					</span>
				</span>
			),
		}),
	});
	return (
		<div className={classes(styles.row, styles.compact, styles.drawer)}>
			{toggle}
			{(shown !== undefined || onHover !== undefined) && (
				<span className={styles.end}>
					<span className={styles.actions} data-actions>
						{onHover !== undefined && <span className={styles.hoverActions}>{onHover}</span>}
						{shown}
					</span>
				</span>
			)}
		</div>
	);
};

/**
 * A drawer at the foot of the sidebar and what it holds: its {@link DrawerItem}, and while it's
 * open, its `children` under it, such as a card of the hidden projects or of the terminals grouped
 * by what they're at. It takes the row's props, and the host keeps whether it's `folded`.
 *
 * A `--but-border-subtle` hairline rules it off from what's above, the cards or another drawer,
 * an open one's card too. The last drawer at the panel's foot takes `last`, which rules it off
 * from the foot with a hairline under it.
 *
 * @figma `folded=false` shows the `content` slot: put the card in it.
 * @import import { Drawer } from "@gitbutler/but-ui-react/SidebarRow.tsx";
 */
export const Drawer: FC<
	{
		name: string;
		count?: number;
		folded: boolean;
		onToggleFolded?: () => void;
		actions?: ReactNode;
		hoverActions?: ReactNode;
		children?: ReactNode;
		/** The drawer at the panel's foot: a hairline under it too. */
		last?: boolean;
	} & Omit<ComponentProps<"section">, "children">
> = ({ name, count, folded, onToggleFolded, actions, hoverActions, children, last, ...props }) => (
	<section
		aria-label={name}
		{...props}
		className={classes(styles.drawerSection, last && styles.drawerLast, props.className)}
	>
		<DrawerItem
			name={name}
			count={count}
			folded={folded}
			onToggleFolded={onToggleFolded}
			actions={actions}
			hoverActions={hoverActions}
		/>
		{!folded && filled(children) !== undefined && (
			<div className={styles.drawerContent}>{children}</div>
		)}
	</section>
);

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
					className={classes(leadingRow ? "text-14" : "text-13", styles.name)}
				>
					{name}
				</Name>
			}
		/>
	);
};

/**
 * A row still loading, in its place: a bone where its glyph goes and one for its name (`width`),
 * at a row's height, so the card holds its shape and nothing moves as the rows land. Draw a few, of
 * different widths, as the rows a card usually holds. It takes no pointer and says nothing to a
 * screen reader: mark what holds the rows `aria-busy`.
 *
 * @import import { LoadingItem } from "@gitbutler/but-ui-react/SidebarRow.tsx";
 */
export const LoadingItem: FC<
	{ width: number } & Pick<RowProps, "depth" | "guides"> & Omit<ComponentProps<"div">, "children">
> = ({ width, ...props }) => (
	<Row
		{...props}
		aria-hidden
		name=""
		icon={<SkeletonBone shape="circle" size={14} />}
		label={<SkeletonBone className="text-11" width={width} />}
		rowClassName={styles.loading}
	/>
);

/**
 * Where rows should be and can't show, in their place: what failed (`message`, one sentence) under
 * the warning glyph, in grey, and the way out (`action`, such as Try again) as a link after it. The
 * reason, when there is one beyond the message, goes in `tooltip`. Rows that load from somewhere
 * the card's own data doesn't, such as a branch's commits on its remote, use it rather than open
 * onto nothing.
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
