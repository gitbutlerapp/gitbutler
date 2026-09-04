import { remoteTrackingLabel } from "#ui/branch.ts";
import type { RefInfo, Stack, TargetCommit, TargetCommitPage } from "@gitbutler/but-sdk";

/*
 * The stacks section as a graph: which line each card's base takes, which
 * rows the upstream section shows, and the rail paths between them. Pure:
 * pixels come in as measured anchors and go out as SVG path strings.
 *
 * The rules: every card sits on one rail, which runs on below the last card
 * as the trunk rail. The lines the cards fork from run to its left, a gap
 * apart from the edge in: the parent line nearest the rail, other bases'
 * columns beyond it, deepest outermost; where they pass behind a card they
 * are drawn faint and dashed. Each card's rail bends off its floor onto its line in the
 * gap below, except the last card's, which runs straight on. Above the
 * upstream header the lines turn onto one row into the trunk rail, which
 * runs through the header, with the incoming commits on a leg beside it,
 * into the base header's chevron and, unfolded, out into the base rows.
 */

/** Lines sit this far apart, and from the cards' rail. */
const BEHIND_GAP = 12;
/** The outermost line's distance from the edge. */
const EDGE_GAP = 12;
/** With no line to keep clear of, the rail sits this far in from the edge gap: air, not a line's room. */
const LONE_GAP = BEHIND_GAP / 2;
/** A rail's x inside a row: the row inset plus half the glyph. Keep in sync with Row.module.css. */
const GLYPH_X = 20;
/** The rows' own inset, Row.module.css's default. */
const ROW_INSET = 12;
/** The row inset that puts a row's glyph on the line at `x`. */
export const rowInsetFor = (x: number): number => x - (GLYPH_X - ROW_INSET);
/** Every turn is a quarter circle of this radius. */
const CORNER_R = 4;
/** How far the rails keep from the header's chevron centre: half its icon and a hair of air. */
const CHEVRON_CLEARANCE = 10;
/**
 * Vertical gap between cards, where a card's rail bends onto its line. Every
 * stretch the lines bend through is this tall: the room above the upstream
 * header, the room under it while folded, and the room around the incoming
 * card. Keep in sync with TrunkSection.module.css.
 */
export const CARD_GAP = 20;
/** How many of an incoming run show before the rest fold. */
const INCOMING_PREVIEW = 3;

type Folds = {
	/** The upstream header's fold: the commits incoming from the target. */
	incomingExpanded: boolean;
	/** The base header's fold: the base rows and the older history. */
	baseExpanded: boolean;
	expandedRuns: ReadonlySet<string>;
	/** Whether the history below the deepest fork point has been asked for. */
	olderShown: boolean;
};

export type TrunkRun = {
	kind: "run";
	id: string;
	incoming: boolean;
	/** Always shown. */
	preview: Array<TargetCommit>;
	/** Shown only while the run is unfolded; empty when nothing folds. */
	rest: Array<TargetCommit>;
	expanded: boolean;
};

export type TrunkItem = { kind: "fork"; commit: TargetCommit } | TrunkRun;

export type GutterPlan = {
	/** Card order as indices into the stacks given: by base depth, then as given. */
	order: Array<number>;
	/** Each card's base rail column, in card order. */
	columnOf: Array<number>;
	/** Every card's rail x, a gap right of the lines that run behind the cards; the trunk rail's too. */
	railX: number;
	/**
	 * The columns some card bends onto, outermost first: other bases' by
	 * depth, then the parent line, the spine's, next to the rail. Only these
	 * get a line and the room for one; with none, the rail sits half a gap in.
	 */
	lines: Array<number>;
	/** The spine's column: the base most stacks sit on, which runs on as the trunk rail. */
	spine: number;
	header: { label: string; incoming: number };
	incomingExpanded: boolean;
	baseExpanded: boolean;
	/** The commit the stacks nearest the tip sit on, which the base header names; null while unknown. */
	base: TargetCommit | null;
	/**
	 * Commits on the target the workspace does not have yet: they sit ahead of
	 * the base on their own leg beside the spine. Empty while folded.
	 */
	incoming: Array<TrunkRun>;
	/** The base and below, on the spine; empty while the base is folded. */
	trunk: Array<TrunkItem>;
	/** Older history below the deepest fork point, once asked for; plain rows on the trunk. Empty while the base is folded. */
	older: Array<TargetCommit>;
	/** Older commits already listed but not shown yet: the first "show more" reveals these. */
	olderHidden: number;
};

/**
 * A stretch of the target line: a commit a workspace stack forks from, or
 * the run of commits between such fork points.
 */
type TargetItem =
	| { type: "fork"; commit: TargetCommit }
	| { type: "run"; commits: Array<TargetCommit>; inWorkspace: boolean };

/**
 * Cut the target line at the workspace's fork points: each stack's base
 * stands on its own, and the commits between them group into maximal runs
 * sharing one relation to the workspace.
 */
const segmentAtForks = (
	commits: ReadonlyArray<TargetCommit>,
	stacks: ReadonlyArray<Stack>,
): Array<TargetItem> => {
	const forks = new Set(stacks.flatMap((stack) => (stack.base === null ? [] : [stack.base])));
	const items: Array<TargetItem> = [];
	for (const commit of commits) {
		if (forks.has(commit.commit.id)) {
			items.push({ type: "fork", commit });
			continue;
		}
		const last = items.at(-1);
		if (last?.type === "run" && last.inWorkspace === commit.inWorkspace) last.commits.push(commit);
		else items.push({ type: "run", commits: [commit], inWorkspace: commit.inWorkspace });
	}
	return items;
};

/** The line down to the deepest fork point, its runs folded as the fold state says. */
const foldTrunk = (line: ReadonlyArray<TargetItem>, folds: Folds): Array<TrunkItem> =>
	line.slice(0, line.findLastIndex((item) => item.type === "fork") + 1).map((item) => {
		if (item.type === "fork") return { kind: "fork", commit: item.commit };
		// Incoming runs keep a few newest in view; runs the workspace already
		// has fold entirely. A fold hiding a single row is not worth the row.
		const incoming = !item.inWorkspace;
		const previewCount = incoming ? INCOMING_PREVIEW : 0;
		const folded = item.commits.length - previewCount > 1;
		const id = item.commits[0]?.commit.id ?? "";
		return {
			kind: "run",
			id,
			incoming,
			preview: folded ? item.commits.slice(0, previewCount) : item.commits,
			rest: folded ? item.commits.slice(previewCount) : [],
			expanded: folds.expandedRuns.has(id),
		};
	});

export const planGutter = (
	stacks: ReadonlyArray<Stack>,
	target: RefInfo["target"],
	trunk: TargetCommitPage | undefined,
	folds: Folds,
	/** Pages of history below the listing, in order, as loaded. */
	olderPages: ReadonlyArray<TargetCommit> = [],
): GutterPlan => {
	const commits = trunk?.commits ?? [];
	const line = segmentAtForks(commits, stacks);
	const items = foldTrunk(line, folds);
	// What the listing holds below the deepest fork point heads the older
	// history, before any page fetched for it.
	const trailing = line
		.slice(line.findLastIndex((item) => item.type === "fork") + 1)
		.flatMap((item) => (item.type === "fork" ? [item.commit] : item.commits));
	const baseItems = items.filter((item) => item.kind !== "run" || !item.incoming);

	// Columns by base depth, deepest nearest the edge. Bases the listing does
	// not reach are deeper than anything listed and take the innermost column.
	// The spine is the column most stacks sit on (the newest on a tie): it is
	// the cards' own rail and runs on as the trunk rail, and only stacks on
	// another base get a column of their own, left of the cards.
	const forkOrder = items.flatMap((item) => (item.kind === "fork" ? [item.commit.commit.id] : []));
	const orphans = stacks.some((stack) => stack.base === null || !forkOrder.includes(stack.base));
	const shift = orphans ? 1 : 0;
	const baseColumn = new Map(
		forkOrder.map((id, index) => [id, forkOrder.length - 1 - index + shift]),
	);
	const columns = Math.max(1, forkOrder.length + shift);
	const columnOfStack = (stack: Stack): number =>
		(stack.base === null ? undefined : baseColumn.get(stack.base)) ?? 0;
	const stacksInColumn = new Map<number, number>();
	for (const stack of stacks) {
		const column = columnOfStack(stack);
		stacksInColumn.set(column, (stacksInColumn.get(column) ?? 0) + 1);
	}
	let spine = columns - 1;
	for (const [column, count] of stacksInColumn) {
		const best = stacksInColumn.get(spine) ?? 0;
		if (count > best || (count === best && column > spine)) spine = column;
	}

	// Cards on another base come first, by depth; the spine's cards follow,
	// since the spine starts at the first of them and runs through the rest.
	const order = stacks
		.map((stack, index) => ({ index, column: columnOfStack(stack) }))
		.sort((a, b) => {
			const ka = a.column === spine ? Infinity : a.column;
			const kb = b.column === spine ? Infinity : b.column;
			return ka === kb ? a.index - b.index : ka - kb;
		});
	// Every card bends onto its column's line, except the last card, whose
	// rail runs straight on as the trunk when it sits on the spine.
	const bending = order.filter(
		(entry, position) => position < order.length - 1 || entry.column !== spine,
	);
	const lines = [...new Set(bending.map((entry) => entry.column))].sort((a, b) =>
		a === spine ? 1 : b === spine ? -1 : a - b,
	);

	return {
		order: order.map((entry) => entry.index),
		columnOf: order.map((entry) => entry.column),
		railX: EDGE_GAP + (lines.length === 0 ? LONE_GAP : lines.length * BEHIND_GAP),
		lines,
		spine,
		header: {
			label: target ? remoteTrackingLabel(target.remoteTrackingRef) : "target",
			incoming: commits.filter((entry) => !entry.inWorkspace).length,
		},
		incomingExpanded: folds.incomingExpanded,
		baseExpanded: folds.baseExpanded,
		base: baseItems.find((item) => item.kind === "fork")?.commit ?? null,
		incoming: folds.incomingExpanded
			? items.flatMap((item) => (item.kind === "run" && item.incoming ? [item] : []))
			: [],
		trunk: folds.baseExpanded ? baseItems : [],
		older: folds.baseExpanded && folds.olderShown ? [...trailing, ...olderPages] : [],
		olderHidden: folds.olderShown ? 0 : trailing.length + olderPages.length,
	};
};

/* ------------------------------------------------------------------ rails */

/** Measured positions the rails run between, in the scroller's content pixels. */
type Span = { topY: number; bottomY: number };

type Anchors = {
	/** Per card in plan order: its top and bottom edges and where its last row's rail glyph ends; null while unmeasured. */
	cards: Array<(Span & { exitY: number }) | null>;
	/** Where the last card ends, measured or estimated, so the rails reach the section below. */
	cardsEnd: number;
	/** The upstream header row: its glyph's centre and its edges. */
	header: Span & { y: number };
	/** The incoming leg's card, and the rows in it, while unfolded with incoming commits. */
	leg: { card: Span; rows: Span } | null;
	/** The base header row: its chevron's centre and its top edge; null while there is no base. */
	base: { y: number; topY: number } | null;
	/** Where the base rows begin under their header, while unfolded; null while folded. */
	baseRowsTopY: number | null;
};

export type Rail = {
	d: string;
	/** The stretch runs behind a card, and is drawn faint and dashed. */
	through?: boolean;
};

const ARC_K = 0.5523;
const n = (value: number): string => String(Math.round(value * 100) / 100);

/** A quarter-circle turn right, from coming down a line at `x` onto the row at `cy`. */
const turnRight = (x: number, cy: number): string => {
	const y0 = cy - CORNER_R;
	const c1y = y0 + CORNER_R * ARC_K;
	const c2x = x + CORNER_R * (1 - ARC_K);
	return `C ${n(x)} ${n(c1y)} ${n(c2x)} ${n(cy)} ${n(x + CORNER_R)} ${n(cy)}`;
};

/**
 * An S-bend from one line to another: straight, a quarter-turn toward the
 * other line, straight across, a quarter-turn back down, straight on.
 */
const sBend = (x0: number, y0: number, x1: number, y1: number): string => {
	const dir = x1 > x0 ? 1 : -1;
	const r = CORNER_R;
	const my = (y0 + y1) / 2;
	const k = ARC_K * r;
	return [
		`L ${n(x0)} ${n(my - r)}`,
		`C ${n(x0)} ${n(my - r + k)} ${n(x0 + dir * (r - k))} ${n(my)} ${n(x0 + dir * r)} ${n(my)}`,
		`L ${n(x1 - dir * r)} ${n(my)}`,
		`C ${n(x1 - dir * (r - k))} ${n(my)} ${n(x1)} ${n(my + r - k)} ${n(x1)} ${n(my + r)}`,
		`L ${n(x1)} ${n(y1)}`,
	].join(" ");
};

/** A column's line, by its place among the lines drawn, from the edge in. */
const lineX = (plan: Pick<GutterPlan, "lines">, column: number): number =>
	EDGE_GAP + plan.lines.indexOf(column) * BEHIND_GAP;

/** The rail x whose glyphs a row inset of `inset` puts on the line: {@link rowInsetFor} undone. */
export const railXFor = (inset: number): number => inset + (GLYPH_X - ROW_INSET);

/**
 * The incoming leg's line: one gap right of the trunk rail. With commits
 * incoming the target has moved on from the workspace's history, so its
 * header sits on this line, not on the trunk.
 */
export const legX = (plan: Pick<GutterPlan, "railX">): number => plan.railX + BEHIND_GAP;

/**
 * A line straight down from `from` to `to`, drawn as behind each card it
 * passes and solid in the gaps between them.
 */
const runDown = (
	rails: Array<Rail>,
	x: number,
	from: number,
	to: number,
	cards: ReadonlyArray<Span | null>,
): void => {
	let y = from;
	for (const card of cards) {
		if (card === null || card.topY < from || card.topY >= to) continue;
		if (card.topY > y) rails.push({ d: `M ${n(x)} ${n(y)} L ${n(x)} ${n(card.topY)}` });
		rails.push({ through: true, d: `M ${n(x)} ${n(card.topY)} L ${n(x)} ${n(card.bottomY)}` });
		y = card.bottomY;
	}
	if (to > y) rails.push({ d: `M ${n(x)} ${n(y)} L ${n(x)} ${n(to)}` });
};

/**
 * Rails are drawn where they carry information. Every card's rail bends off
 * its floor in the gap below onto its line, to the left, except the last
 * card's, which runs straight on as the trunk rail. The lines run behind the
 * cards below, faint and dashed, and solid in the gaps. Above the upstream header every
 * line turns right onto one row that runs into the trunk rail, which goes on:
 * into the header's chevron when the target is an ancestor of the workspace,
 * or straight past it to the base rows when the target has moved on, in
 * which case the header sits on the incoming leg's line and only the leg's
 * bottom joins the trunk.
 */
export const railPaths = (plan: GutterPlan, anchors: Anchors): Array<Rail> => {
	const rails: Array<Rail> = [];
	const { cards, cardsEnd, header, leg, base, baseRowsTopY } = anchors;
	if (cards.length === 0) return rails;
	const rail = plan.railX;
	const legLine = legX(plan);
	const branched = plan.header.incoming > 0;

	// The row above the header that the lines turn onto, centred in the gap
	// below the last card.
	const mergeY = cardsEnd + CARD_GAP / 2;

	const lineTop = new Map<number, number>();
	const last = cards.length - 1;
	for (const [position, card] of cards.entries()) {
		if (card === null) continue;
		const column = plan.columnOf[position] ?? plan.spine;
		// The last card's rail runs straight on; the trunk rail picks it up.
		if (position === last && column === plan.spine) {
			rails.push({ d: `M ${n(rail)} ${n(card.exitY)} L ${n(rail)} ${n(cardsEnd)}` });
			continue;
		}
		const joinY = card.bottomY + CARD_GAP;
		lineTop.set(column, Math.min(lineTop.get(column) ?? Infinity, joinY));
		rails.push({
			d: [
				`M ${n(rail)} ${n(card.exitY)}`,
				`L ${n(rail)} ${n(card.bottomY)}`,
				sBend(rail, card.bottomY, lineX(plan, column), joinY),
			].join(" "),
		});
	}

	// Each line down behind the cards, then right onto the row into the trunk rail.
	const lines = [...lineTop].sort(([a], [b]) => lineX(plan, a) - lineX(plan, b));
	for (const [column, top] of lines) {
		const x = lineX(plan, column);
		runDown(rails, x, top, cardsEnd, cards);
		rails.push({
			d: `M ${n(x)} ${n(cardsEnd)} L ${n(x)} ${n(mergeY - CORNER_R)} ${turnRight(x, mergeY)}`,
		});
	}
	const outermost = lines[0];
	if (outermost !== undefined) {
		rails.push({
			d: `M ${n(lineX(plan, outermost[0]) + CORNER_R)} ${n(mergeY)} L ${n(rail)} ${n(mergeY)}`,
		});
	}

	// The trunk rail runs on from the last card: through the ref row, whose
	// glyph carries it, when the target is an ancestor, and straight past the
	// header, behind the incoming card, when it has moved on; then into the
	// base header's chevron and, unfolded, out of it into the base rows,
	// which end it themselves.
	if (!branched) {
		rails.push({ d: `M ${n(rail)} ${n(cardsEnd)} L ${n(rail)} ${n(header.topY)}` });
		if (base) {
			rails.push({
				d: `M ${n(rail)} ${n(header.bottomY)} L ${n(rail)} ${n(base.y - CHEVRON_CLEARANCE)}`,
			});
		}
	} else if (base) {
		runDown(rails, rail, cardsEnd, base.y - CHEVRON_CLEARANCE, [leg?.card ?? null]);
	}
	if (base && baseRowsTopY !== null) {
		rails.push({
			d: `M ${n(rail)} ${n(base.y + CHEVRON_CLEARANCE)} L ${n(rail)} ${n(baseRowsTopY)}`,
		});
	}
	// The leg runs down from the header's chevron and joins the trunk above
	// the base header: out of the bottom of its card, or, folded, in the
	// gap under the header; either way through a card gap, like a card's exit.
	if (branched && base) {
		const from = leg
			? [`M ${n(legLine)} ${n(leg.rows.bottomY)}`, `L ${n(legLine)} ${n(leg.card.bottomY)}`]
			: [
					`M ${n(legLine)} ${n(header.y + CHEVRON_CLEARANCE)}`,
					`L ${n(legLine)} ${n(header.bottomY)}`,
				];
		const bendFrom = leg ? leg.card.bottomY : header.bottomY;
		if (leg) {
			rails.push({
				d: `M ${n(legLine)} ${n(header.y + CHEVRON_CLEARANCE)} L ${n(legLine)} ${n(leg.rows.topY)}`,
			});
		}
		rails.push({ d: [...from, sBend(legLine, bendFrom, rail, base.topY)].join(" ") });
	}

	return rails;
};
