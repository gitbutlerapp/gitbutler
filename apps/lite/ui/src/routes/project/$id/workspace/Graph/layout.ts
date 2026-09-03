import { addressEquals, commitAddress, type Address } from "#ui/addresses.ts";
import { assert } from "#ui/assert.ts";
import { remoteTrackingLabel } from "#ui/branch.ts";
import type { RefInfo, Stack, TargetCommit, TargetCommitPage } from "@gitbutler/but-sdk";

/*
 * The stacks section as a graph: the order the cards come in, which rows
 * the upstream section shows, and the rail paths between them. Pure: pixels
 * come in as measured anchors and go out as SVG path strings.
 *
 * The rules: one main line runs up the left, from the last row of the
 * upstream section through the section and on past every card, on the
 * card's left, into the top card, which sits on it. Every other card forks
 * off it to the right in the gap under the card: its rows sit one gap right
 * of the main line, and its rail bends down onto the main line below its
 * floor. A target that has moved on gets a card like that too: its incoming
 * commits sit on a leg beside the main line, which bends onto it under the
 * card, at the merge base. Nothing else is drawn. The SVG draws only the
 * gaps between the stack cards: every card draws its own rails to its
 * edges, and the section draws its own, in Section.module.css.
 */

/** The forked cards' rail sits this far right of the main line, which runs on behind them. */
const COLUMN_GAP = 12;
/** The main line's x, a gap and a half in from the edge. */
export const MAIN_X = 18;
/** The forked cards' rail, and a moved-on target's: where their row glyphs sit; the top card's sit on the main line. */
export const CARD_X = MAIN_X + COLUMN_GAP;
/** A rail's x inside a row: the row inset plus half the glyph. Keep in sync with Row.module.css. */
const GLYPH_X = 20;
/** The rows' own inset, Row.module.css's default. */
const ROW_INSET = 12;
/** The row inset that puts a row's glyph on the line at `x`. */
export const rowInsetFor = (x: number): number => x - (GLYPH_X - ROW_INSET);
/** Every turn is a quarter circle of this radius. */
const CORNER_R = 4;
/**
 * Vertical gap between cards, where a card's rail bends onto its line. Every
 * stretch the lines bend through is this tall: the room above the upstream
 * header, the room under it while folded, and the room around the incoming
 * card. Keep in sync with Section.module.css.
 */
export const CARD_GAP = 20;
/**
 * The folded target card's height, which a row scrolled into view clears
 * while the card docks: its borders, its air, its header row and its air.
 * Keep in sync with Section.module.css.
 */
export const DOCKED_HEIGHT = 2 + 4 + 28 + 4;
/** A long list, a run or the older history, shows this much at first, and this much more with each ask. */
export const FIRST = 10;
export const MORE = 20;

type Folds = {
	/** The upstream header's fold: the commits incoming from the target. */
	incomingExpanded: boolean;
	/** The base header's fold: the base rows and the older history. */
	baseExpanded: boolean;
	/** How many times more of each run was asked for, by the run's id. */
	moreRuns: Readonly<Record<string, number>>;
	/** How many times more of the older history was asked for since the base opened. */
	moreOlder: number;
};

/**
 * A target commit as a value: the address the graph, the Upstream tab and the
 * applied list agree on. The change id falls back to the commit id, which
 * should be revisited.
 */
export const targetCommitAddress = (commit: TargetCommit): Address =>
	commitAddress({
		commitId: commit.commit.id,
		changeId: commit.commit.changeId ?? commit.commit.id,
	});

export type Run = {
	kind: "run";
	id: string;
	incoming: boolean;
	/** The commits shown, newest first. */
	shown: Array<TargetCommit>;
	/** How many more it holds, folded away below them. */
	hidden: number;
	/** Whether an ask revealed more than shows at first. */
	expanded: boolean;
};

export type Item = { kind: "fork"; commit: TargetCommit } | Run;

export type Plan = {
	/** Card order as indices into the stacks given: by base depth, deepest first, then as given. */
	order: Array<number>;
	/** The target's row: its label and how many commits are incoming. Not a value: nothing selects it. */
	header: { label: string; incoming: number };
	/**
	 * The target's tip is the base itself, with nothing incoming: one row
	 * stands for both, the base commit under the ref's name.
	 */
	refOnBase: boolean;
	incomingExpanded: boolean;
	baseExpanded: boolean;
	/**
	 * The commit the stacks nearest the tip sit on, named by the base header
	 * and left out of the rows under it. Not a value either: the header only
	 * folds. Null while unknown.
	 */
	base: TargetCommit | null;
	/**
	 * Commits on the target the workspace does not have yet: they sit ahead of
	 * the base on their own leg beside the main line. Empty while folded.
	 */
	incoming: Array<Run>;
	/** Below the base, on the main line; empty while the base is folded. */
	belowBase: Array<Item>;
	/** Older history below the deepest fork point, as much of it as is shown: what the listing holds, then the pages fetched; plain rows. Empty while the base is folded. */
	older: Array<TargetCommit>;
	/** Older history loaded but not shown yet: what the next ask reveals before any page is fetched. */
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
const foldRuns = (line: ReadonlyArray<TargetItem>, folds: Folds): Array<Item> =>
	line.slice(0, line.findLastIndex((item) => item.type === "fork") + 1).map((item) => {
		if (item.type === "fork") return { kind: "fork", commit: item.commit };
		// Incoming runs keep their newest in view; runs the workspace already
		// has fold entirely. Each ask reveals more. A fold hiding a single row
		// is not worth the row.
		const incoming = !item.inWorkspace;
		const id = assert(item.commits[0]).commit.id;
		const asked = folds.moreRuns[id] ?? 0;
		const asFar = (incoming ? FIRST : 0) + asked * MORE;
		const count = item.commits.length - asFar <= 1 ? item.commits.length : asFar;
		return {
			kind: "run",
			id,
			incoming,
			shown: item.commits.slice(0, count),
			hidden: item.commits.length - count,
			expanded: asked > 0,
		};
	});

export const layout = (
	stacks: ReadonlyArray<Stack>,
	target: RefInfo["target"],
	listing: TargetCommitPage | undefined,
	folds: Folds,
	/** Pages of history below the listing, in order, as loaded. */
	olderPages: ReadonlyArray<TargetCommit> = [],
): Plan => {
	const commits = listing?.commits ?? [];
	const line = segmentAtForks(commits, stacks);
	const items = foldRuns(line, folds);
	// What the listing holds below the deepest fork point heads the older
	// history, before the pages fetched for it.
	const trailing = line
		.slice(line.findLastIndex((item) => item.type === "fork") + 1)
		.flatMap((item) => (item.type === "fork" ? [item.commit] : item.commits));
	const older = folds.baseExpanded ? [...trailing, ...olderPages] : [];
	const olderShown = FIRST + folds.moreOlder * MORE;
	const baseItems = items.filter((item) => item.kind !== "run" || !item.incoming);
	const base = baseItems.find((item) => item.kind === "fork")?.commit ?? null;
	const belowBase = baseItems.filter(
		(item) => item.kind !== "fork" || item.commit.commit.id !== base?.commit.id,
	);

	// Deeper bases first, nearest their own history's end; a base the listing
	// does not reach is deeper than any listed, all of them equally, so they
	// keep the order given.
	const forkOrder = items.flatMap((item) => (item.kind === "fork" ? [item.commit.commit.id] : []));
	const depth = (stack: Stack): number => {
		const index = stack.base === null ? -1 : forkOrder.indexOf(stack.base);
		return index === -1 ? forkOrder.length : index;
	};
	const order = stacks
		.map((stack, index) => ({ index, stack }))
		.sort((a, b) => {
			const byDepth = depth(b.stack) - depth(a.stack);
			return byDepth === 0 ? a.index - b.index : byDepth;
		});

	return {
		order: order.map((entry) => entry.index),
		header: {
			label: target ? remoteTrackingLabel(target.remoteTrackingRef) : "target",
			incoming: commits.filter((entry) => !entry.inWorkspace).length,
		},
		refOnBase:
			base !== null &&
			commits[0]?.commit.id === base.commit.id &&
			commits.every((entry) => entry.inWorkspace),
		incomingExpanded: folds.incomingExpanded,
		baseExpanded: folds.baseExpanded,
		base,
		incoming: folds.incomingExpanded
			? items.flatMap((item) => (item.kind === "run" && item.incoming ? [item] : []))
			: [],
		belowBase: folds.baseExpanded ? belowBase : [],
		older: older.slice(0, olderShown),
		olderHidden: Math.max(0, older.length - olderShown),
	};
};

const runCommits = (run: Run): Array<TargetCommit> => run.shown;

const itemCommits = (item: Item): Array<TargetCommit> =>
	item.kind === "fork" ? [item.commit] : runCommits(item);

/** The rows a fold shows, as values, top to bottom. The headers are not values. */
export const foldAddresses = (plan: Plan, fold: "incoming" | "base"): Array<Address> =>
	(fold === "incoming"
		? plan.incoming.flatMap(runCommits)
		: [...plan.belowBase.flatMap(itemCommits), ...plan.older]
	).map(targetCommitAddress);

/** The section's rows as values, top to bottom, as the fold state shows them. */
export const sectionAddresses = (plan: Plan): Array<Address> => [
	...foldAddresses(plan, "incoming"),
	...foldAddresses(plan, "base"),
];

/** The review a shown target commit landed, by commit id; null when none or not shown. */
export const planCommitReview = (plan: Plan, commitId: string) =>
	[
		...plan.incoming.flatMap(runCommits),
		...plan.belowBase.flatMap(itemCommits),
		...plan.older,
	].find((commit) => commit.commit.id === commitId)?.review ?? null;

/** Which of the section's folds a row sits in, so a fold key on the row can close it; null off the section. */
export const foldAt = (plan: Plan, address: Address): "incoming" | "base" | null => {
	const inFold = (fold: "incoming" | "base") =>
		foldAddresses(plan, fold).some((other) => addressEquals(address, other));
	return inFold("incoming") ? "incoming" : inFold("base") ? "base" : null;
};

/* ------------------------------------------------------------------ rails */

/** A card's edges from the virtualiser, in the scroller's content pixels. */
export type Card = { topY: number; bottomY: number };

const ARC_K = 0.5523;
const n = (value: number): string => String(Math.round(value * 100) / 100);

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

/** The gap under the target's card, which its leg bends through. Keep in sync with Section.module.css. */
const LEG_GAP = 12;
/** The leg's bend under the target's card: off the card's floor, onto the main line at the merge base header's top. */
export const LEG_BEND = `M ${n(CARD_X)} 0 ${sBend(CARD_X, 0, MAIN_X, LEG_GAP)}`;

/** A line straight down from `from` to `to`. */
const runDown = (rails: Array<string>, x: number, from: number, to: number): void => {
	if (to > from) rails.push(`M ${n(x)} ${n(from)} L ${n(x)} ${n(to)}`);
};

/**
 * The rails through the gaps between the cards, as SVG paths; every card
 * draws its own to its edges. The top card sits on the main line, which runs
 * on from its floor, through each gap into the card below, and through the
 * gap under the last card to the upstream section's first row; from there
 * the section carries it itself. Every other card's rail bends off its
 * floor onto the main line in the gap below it.
 */
export const rails = (cards: ReadonlyArray<Card>, cardsEnd: number): Array<string> => {
	const paths: Array<string> = [];
	const [top, ...rest] = cards;
	if (top === undefined) return paths;

	let y = top.bottomY;
	for (const card of rest) {
		paths.push(
			`M ${n(CARD_X)} ${n(card.bottomY)} ${sBend(CARD_X, card.bottomY, MAIN_X, card.bottomY + CARD_GAP)}`,
		);
		runDown(paths, MAIN_X, y, card.topY);
		y = card.bottomY;
	}
	runDown(paths, MAIN_X, y, cardsEnd + CARD_GAP);
	return paths;
};
