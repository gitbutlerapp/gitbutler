import { Fragment, useState, type FC, type ReactNode } from "react";
import { flushSync } from "react-dom";
import { stories } from "../stories/stories.ts";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import { MetaCount, MetaStatus } from "./MetaCount.tsx";
import { EmptyState } from "./EmptyState.tsx";
import {
	BranchItem,
	CommitItem,
	Drawer,
	DrawerItem,
	MoreItem,
	RepoItem,
	RowDivider,
	UncommittedItem,
	WorktreeItem,
} from "./SidebarRow.tsx";
import { Column, push, RepoCard, TerminalGroup, update } from "./story-assets/workspaceTree.tsx";
import { Tooltip } from "./Tooltip.tsx";

const core = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=${node}`;

const meta = stories({
	title: "Sidebar/Drawer",
	id: "components-drawer",
	component: Drawer,
	parameters: { design: { type: "figma", url: core("5403-16821") }, width: 334 },
});

/** The card a drawer holds, on the panel's ground under its row. */
const Card: FC<{ children: ReactNode }> = ({ children }) => (
	<div
		style={{
			overflow: "hidden",
			border: "1px solid var(--but-border-subtle)",
			borderRadius: "var(--but-radius-card)",
			backgroundColor: "var(--but-background)",
		}}
	>
		{children}
	</div>
);

/** A drawer's card with nothing in it: the empty state, with the card's room around it. */
const EmptyCard: FC<{ children: ReactNode }> = ({ children }) => (
	<Card>
		<div style={{ padding: "28px 16px" }}>{children}</div>
	</Card>
);

/** A ghost icon button at a drawer's end, with its tooltip. */
const ghost = (icon: "plus" | "mixer", label: string) => (
	<Tooltip key={label} content={label}>
		<Button variant="ghost" iconOnly aria-label={label}>
			<Icon name={icon} />
		</Button>
	</Tooltip>
);

const blocked = (
	<Tooltip content="Command not found">
		<MetaStatus variant="failed" aria-label="Command not found" />
	</Tooltip>
);

/** Puts a project the sidebar hides back among its cards: its row's `glyphAction`, an open eye
 * while it's hidden, as a press shows it, under the pointer on the glyph. */
const show = (name: string) => (
	<Tooltip content="Unhide project">
		<Button variant="ghost" size="small" iconOnly aria-label={`Show ${name}`}>
			<Icon name="eye" />
		</Button>
	</Tooltip>
);

/** A drawer, open: its row, its buttons, and the card it holds under it. */
export const Default = meta.story({
	args: {
		name: "Terminals",
		icon: "terminal-copy",
		count: 2,
		folded: false,
		actions: [ghost("mixer", "Group terminals"), ghost("plus", "New terminal…")],
		children: (
			<Card>
				<TerminalGroup
					label="Idle"
					terminals={[
						{ name: "review-cards", place: "gpui-row-buttons" },
						{ name: "Terminal", place: "Main worktree" },
					]}
					last
				/>
			</Card>
		),
	},
});

/** The terminals, grouped in their card by what they're at. */
export const GroupedTerminals = meta.story({
	parameters: { design: { type: "figma", url: core("5403-18924") } },
	args: {
		name: "Terminals",
		icon: "terminal-copy",
		count: 3,
		folded: false,
		actions: [ghost("mixer", "Group terminals"), ghost("plus", "New terminal…")],
		children: (
			<Card>
				<TerminalGroup
					label="Idle"
					terminals={[
						{ name: "review-cards", place: "gpui-row-buttons" },
						{ name: "Terminal", place: "Main worktree" },
					]}
				/>
				<TerminalGroup
					label="Blocked"
					terminals={[{ name: "codex", place: "Main worktree", meta: blocked }]}
					last
				/>
			</Card>
		),
	},
});

/** The terminals' drawer with nothing in it: its card says where a new one would open. */
export const Empty = meta.story({
	args: {
		name: "Terminals",
		icon: "terminal-copy",
		count: 0,
		folded: false,
		actions: [ghost("mixer", "Group terminals"), ghost("plus", "New terminal…")],
		last: true,
		children: (
			<EmptyCard>
				<EmptyState
					variant="tidy"
					title="No terminals open"
					description="New ones open in gitbutler / Main worktree"
				>
					<Button variant="outline" size="medium">
						<Icon name="plus" />
						New terminal
					</Button>
				</EmptyState>
			</EmptyCard>
		),
	},
});

/** A drawer that keeps whether it's open, as the host does. */
const Open: FC<
	Omit<Parameters<typeof Drawer>[0], "folded" | "onToggleFolded"> & { open?: boolean }
> = ({ open = false, ...props }) => {
	const [folded, setFolded] = useState(!open);
	return <Drawer {...props} folded={folded} onToggleFolded={() => setFolded(!folded)} />;
};

/** A part of the hidden projects' card: one project, ruled off from what's above it across the
 * card, as the drawer's projects are. */
const Part: FC<{ children: ReactNode }> = ({ children }) => (
	<div
		style={{
			display: "flex",
			flexDirection: "column",
			gap: 1,
			padding: 8,
			borderTop: "1px solid var(--but-border-subtle)",
		}}
	>
		{children}
	</div>
);

const age = (value: string) => <MetaCount type="age">{value}</MetaCount>;

/** The hidden projects listed folded, by what they hold, each with its way back into the sidebar. */
const hidden = [
	{
		name: "gitbutler-docs",
		meta: [
			["behind", "7"],
			["branch", "1"],
		],
	},
	{
		name: "homebrew-tap",
		meta: [
			["unpushed", "1"],
			["branch", "2"],
		],
	},
	{ name: "dnd-notes", meta: [["branch", "1"]] },
] as const;

/** The hidden projects' card's head, as 🍑 But UI's: its `DrawerItem` on the canvas, in the card's
 * padding. A press anywhere on it, its padding too, folds the card, as on the Terminals drawer. */
const HiddenHead: FC<{ count: number; folded: boolean; onToggleFolded?: () => void }> = ({
	count,
	folded,
	onToggleFolded,
}) => (
	// oxlint-disable-next-line jsx_a11y/click-events-have-key-events, jsx_a11y/no-static-element-interactions -- Its padding widens the press; the keys reach the DrawerItem's own button.
	<div
		onClick={(event) => {
			if (!(event.target as Element).closest("button")) onToggleFolded?.();
		}}
		style={{
			padding: 8,
			backgroundColor: "var(--but-canvas)",
			cursor: onToggleFolded ? "pointer" : undefined,
		}}
	>
		<DrawerItem
			name="Hidden projects"
			icon="eye-closed"
			count={count}
			folded={folded}
			onToggleFolded={onToggleFolded}
		/>
	</div>
);

/**
 * The hidden projects as the last card of the list, scrolling with the cards above it
 * rather than pinned at the panel's foot, where Terminals stays. A `DrawerItem` heads the card on
 * the canvas, its name, how many projects it holds and its chevron, the whole head pressable as the
 * Terminals drawer is, folded until opened, so it costs one row while nobody looks. Open, a line across the card parts
 * one project from the next, and a project opened in place keeps its own `RowDivider` under its
 * `RepoItem`, so the two lines never read as the same.
 */
const HiddenCard: FC<{ open?: boolean; opened?: boolean }> = ({ open = false, opened = false }) => {
	const [folded, setFolded] = useState(!open);
	const [butDev, setButDev] = useState(opened);
	return (
		<section
			aria-label="Hidden projects"
			style={{
				flexShrink: 0,
				overflow: "hidden",
				border: "1px solid var(--but-border-subtle)",
				borderRadius: "var(--but-radius-card)",
				backgroundColor: "var(--but-background)",
			}}
		>
			<HiddenHead count={24} folded={folded} onToggleFolded={() => setFolded(!folded)} />
			{!folded && (
				<>
					<Part>
						<RepoItem
							name="but-dev"
							folded={!butDev}
							onToggleFolded={() => setButDev(!butDev)}
							meta={
								butDev ? undefined : (
									<>
										<MetaCount type="uncommitted">4</MetaCount>
										<MetaCount type="unpushed">2</MetaCount>
										<MetaCount type="branch">3</MetaCount>
									</>
								)
							}
							glyphAction={show("but-dev")}
						/>
						{butDev && (
							<>
								<RowDivider />
								<Column>
									<WorktreeItem main name="Main worktree" folded={false} meta={age("3d")} />
									<UncommittedItem count={4} depth={1} />
									<BranchItem title="review-cards" depth={1} folded actions={push(2)} />
									<BranchItem
										title="perf: cheaper diff stats"
										pr={{ number: 410, state: "open" }}
										depth={1}
										folded={false}
										meta={
											<>
												<MetaStatus variant="working">CI</MetaStatus>
												{age("8m")}
											</>
										}
									/>
									<CommitItem message="feat: 28px hunk separator" depth={2} pushed={false} />
									<CommitItem message="docs: handoff notes" depth={2} />
									<WorktreeItem name="review-ios" folded meta={age("1d")} actions={update(3)} />
								</Column>
							</>
						)}
					</Part>
					{hidden.map(({ name, meta: counts }) => (
						<Part key={name}>
							<RepoItem
								name={name}
								folded
								onToggleFolded={() => {}}
								meta={
									<>
										{counts.map(([type, value]) => (
											<MetaCount key={type} type={type}>
												{value}
											</MetaCount>
										))}
									</>
								}
								glyphAction={show(name)}
							/>
						</Part>
					))}
					<Part>
						<MoreItem label="20 more" />
					</Part>
				</>
			)}
		</section>
	);
};

/** A sidebar column: the cards scrolling, the hidden projects' card last among them, and the
 * Terminals drawer pinned at the foot. */
const Column334: FC<{ children: ReactNode }> = ({ children }) => (
	<div style={{ display: "flex", flexDirection: "column", width: 334, height: 720 }}>
		<div
			style={{
				display: "flex",
				flexDirection: "column",
				gap: 8,
				flex: 1,
				minHeight: 0,
				overflowY: "auto",
				paddingBottom: 8,
			}}
		>
			<RepoCard
				label="berlin-brutalism"
				header={<RepoItem name="berlin-brutalism" folded={false} meta={age("3d")} />}
			>
				<BranchItem
					title="cursor-unique-names"
					folded
					meta={
						<>
							<MetaCount type="commits">23</MetaCount>
							{age("2h")}
						</>
					}
				/>
			</RepoCard>
			{children}
		</div>
		<Open
			name="Terminals"
			icon="terminal-copy"
			count={5}
			actions={[ghost("mixer", "Group terminals"), ghost("plus", "New terminal…")]}
			last
		/>
	</div>
);

/**
 * Mock: the hidden projects unpinned, the last card of the list, folded (left) and open with a
 * project opened in place (right). Terminals stays a drawer pinned at the foot, its buttons always
 * shown.
 */
export const HiddenProjectsCard = meta.story({
	parameters: { width: undefined },
	render: () => (
		<div style={{ display: "flex", flexWrap: "wrap", gap: 24, alignItems: "flex-start" }}>
			<Column334>
				<HiddenCard />
			</Column334>
			<Column334>
				<HiddenCard open opened />
			</Column334>
		</div>
	),
});

/** The projects of the hide and show motion, in the order the sidebar lists them. */
const travellers = [
	{ name: "but-dev", meta: <MetaCount type="behind">6</MetaCount> },
	{ name: "gitbutler", meta: <MetaCount type="uncommitted">2</MetaCount> },
	{ name: "repo", meta: <MetaCount type="unpushed">3</MetaCount> },
	{ name: "ASCII editor", meta: undefined },
	{ name: "berlin-brutalism", meta: <MetaCount type="branch">2</MetaCount> },
	{ name: "blogerator", meta: undefined },
] as const;

/** The eye on a project's glyph, which moves it to the other side, showing what a press does:
 * closed while it shows, as a press hides it, and open while it's hidden. */
const travelEye = (name: string, hidden: boolean, onPress: (from: HTMLElement) => void) => (
	<Tooltip content={hidden ? "Unhide project" : "Hide project"}>
		<Button
			variant="ghost"
			size="small"
			iconOnly
			aria-label={`${hidden ? "Show" : "Hide"} ${name}`}
			onClick={(event) => onPress(event.currentTarget)}
		>
			<Icon name={hidden ? "eye" : "eye-closed"} />
		</Button>
	</Tooltip>
);

/** A but-ui duration token in ms, as the page's theme resolves it. */
const ms = (token: string) =>
	Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue(token));

/** How long what moves takes, and how it eases: `--but-transition-slow` on the default `ease`,
 * as the GPUI navigator's reorders take it (its `REORDER`), so a project hidden or shown moves as
 * cards trading places do. A small fade takes `--but-transition-fast`. */
const reorder = (): KeyframeAnimationOptions => ({
	duration: ms("--but-transition-slow"),
	easing: "ease",
});
const fast = (): KeyframeAnimationOptions => ({
	duration: ms("--but-transition-fast"),
	easing: "ease",
});

/** How long the project takes to go from `was` to `is`: as a reorder does for a short hop, and
 * longer the further it goes, up to half as long again, so going across the sidebar doesn't
 * jump. */
const travelTime = (was: DOMRect, is: DOMRect) => {
	const slow = ms("--but-transition-slow");
	return Math.min(1.5 * slow, slow + Math.abs(is.top - was.top) * 0.4);
};

/** Where each part of the sidebar that moves is, by its `data-flip` key. */
const measure = (root: HTMLElement) =>
	new Map(
		Array.from(root.querySelectorAll<HTMLElement>("[data-flip]"), (el) => [
			el.dataset.flip ?? "",
			el.getBoundingClientRect(),
		]),
	);

/**
 * Carries the project `moving` from where it was to where it is now, as one copy of its new
 * self over everything, its real self unseen until the copy gets there. Hiding, its card's edge
 * gives way as it goes into its part of the hidden card; showing, it's a card all the way.
 */
const carry = (root: HTMLElement, el: HTMLElement, was: DOMRect, is: DOMRect, hiding: boolean) => {
	const copy = el.cloneNode(true) as HTMLElement;
	copy.removeAttribute("data-flip");
	Object.assign(copy.style, {
		position: "fixed",
		margin: "0",
		zIndex: "10",
		overflow: "hidden",
		pointerEvents: "none",
		boxSizing: "border-box",
		backgroundColor: "var(--but-background)",
	});
	root.append(copy);
	el.style.visibility = "hidden";
	const at = (rect: DOMRect) => ({
		left: `${rect.left}px`,
		top: `${rect.top}px`,
		width: `${rect.width}px`,
		height: `${rect.height}px`,
	});
	// Drawn inside it, taking no room, so what's in it sits where the real row puts it and doesn't
	// shift a pixel as it hands over.
	const edge = (carded: boolean) =>
		hiding
			? {
					boxShadow: `inset 0 0 0 1px ${carded ? "var(--but-border-subtle)" : "transparent"}`,
					borderRadius: carded ? "var(--but-radius-card)" : "0px",
				}
			: { borderRadius: "var(--but-radius-card)" };
	const done = () => {
		copy.remove();
		el.style.visibility = "";
	};
	copy
		.animate(
			[
				{ ...at(was), ...edge(true) },
				{ ...at(is), ...edge(!hiding) },
			],
			{
				...reorder(),
				duration: travelTime(was, is),
				fill: "forwards",
			},
		)
		.finished.then(done, done);
};

/**
 * Moves the sidebar from where things were (`before`) to where they are: each card and part
 * slides from its old place, the hidden card grows or shrinks as it moves, a new divider shows
 * at once, and the project that crossed is carried across.
 */
const slide = (
	root: HTMLElement,
	before: Map<string, DOMRect>,
	moving: string,
	hiding: boolean,
) => {
	const after = measure(root);
	const card = root.querySelector<HTMLElement>('[data-flip="hidden-card"]');
	const [cardWas, cardIs] = [before.get("hidden-card"), after.get("hidden-card")];
	const cardBy = cardWas && cardIs ? cardWas.top - cardIs.top : 0;
	if (card && cardWas && cardIs) {
		card.animate(
			[
				{ transform: `translateY(${cardBy}px)`, height: `${cardWas.height}px` },
				{ transform: "none", height: `${cardIs.height}px` },
			],
			reorder(),
		);
	} else if (card) {
		card.animate([{ opacity: 0 }, { opacity: 1 }], reorder());
	}
	for (const el of root.querySelectorAll<HTMLElement>("[data-flip]")) {
		const key = el.dataset.flip ?? "";
		const [was, is] = [before.get(key), after.get(key)];
		if (key === "hidden-card" || !is) continue;
		if (key === `project-${moving}`) {
			if (was) carry(root, el, was, is, hiding);
			continue;
		}
		if (!was) {
			el.animate([{ opacity: 0 }, { opacity: 1 }], fast());
			continue;
		}
		// What's in the hidden card moves with it already.
		const by = was.top - is.top - (el.dataset.flipIn === undefined ? 0 : cardBy);
		if (by !== 0)
			el.animate([{ transform: `translateY(${by}px)` }, { transform: "none" }], reorder());
	}
};

/** The sidebar of the hide and show motion, keeping which projects are hidden. */
const HideAndShow: FC = () => {
	const [hidden, setHidden] = useState<ReadonlySet<string>>(
		new Set(["ASCII editor", "berlin-brutalism", "blogerator"]),
	);
	const toggle = (name: string, from: HTMLElement) => {
		const hiding = !hidden.has(name);
		const next = new Set(hidden);
		if (hiding) next.add(name);
		else next.delete(name);
		const el = from.closest<HTMLElement>("[data-hide-and-show]");
		const still = globalThis.matchMedia("(prefers-reduced-motion: reduce)").matches;
		if (!el || still) return setHidden(next);
		// Its own moves only: the names' fades are CSS animations of their own.
		for (const animation of el.getAnimations({ subtree: true }))
			if (!(animation instanceof CSSAnimation)) animation.finish();
		const before = measure(el);
		flushSync(() => setHidden(next));
		slide(el, before, name, hiding);
	};
	const row = (project: (typeof travellers)[number], isHidden: boolean) => (
		<RepoItem
			name={project.name}
			folded
			onToggleFolded={() => {}}
			meta={project.meta}
			glyphAction={travelEye(project.name, isHidden, (from) => toggle(project.name, from))}
		/>
	);
	const shown = travellers.filter((project) => !hidden.has(project.name));
	const tucked = travellers.filter((project) => hidden.has(project.name));
	return (
		<div
			data-hide-and-show
			style={{ display: "flex", flexDirection: "column", gap: 8, width: 334 }}
		>
			{shown.map((project) => (
				<div key={project.name} data-flip={`project-${project.name}`}>
					<RepoCard label={project.name} header={row(project, false)}>
						{null}
					</RepoCard>
				</div>
			))}
			<section
				aria-label="Hidden projects"
				data-flip="hidden-card"
				style={{
					overflow: "hidden",
					border: "1px solid var(--but-border-subtle)",
					borderRadius: "var(--but-radius-card)",
					backgroundColor: "var(--but-background)",
				}}
			>
				<HiddenHead count={tucked.length} folded={false} />
				{tucked.map((project) => (
					<Fragment key={project.name}>
						<div
							data-flip={`divider-${project.name}`}
							data-flip-in
							style={{ borderTop: "1px solid var(--but-border-subtle)" }}
						/>
						<div data-flip={`project-${project.name}`} data-flip-in style={{ padding: 8 }}>
							{row(project, true)}
						</div>
					</Fragment>
				))}
			</section>
		</div>
	);
};

/**
 * Mock, hide and show in motion: press a project's eye (its glyph, under the pointer) and it goes
 * across to the hidden projects' card, or back up among the cards, from where it was to where it
 * lands. The cards it leaves close over its room and the card it goes to opens for it, at the same
 * time, each sliding as itself rather than as a picture of itself, so nothing stretches or fades
 * into anything else. Hiding, its card's edge gives way as it becomes a part of the hidden card.
 */
export const HideAndShowMotion = meta.story({
	parameters: { width: 334 },
	render: () => <HideAndShow />,
});
