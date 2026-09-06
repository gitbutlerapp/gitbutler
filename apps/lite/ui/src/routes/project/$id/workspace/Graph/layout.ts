import { addressEquals, commitAddress, type Address } from "#ui/addresses.ts";
import { assert } from "#ui/assert.ts";
import { remoteTrackingLabel } from "#ui/branch.ts";
import type { RefInfo, Stack, TargetCommit, TargetCommitPage } from "@gitbutler/but-sdk";

/*
 * The stacks section as a graph: card order, which section rows show, and
 * the rail paths between the cards. Pure: measured pixels in, SVG paths out.
 *
 * One main line runs up the left into the top card. Every other card, and a
 * moved-on target's, sits a gap to its right and bends onto the line in the
 * gap under it. The SVG draws only the gaps between stack cards; cards and
 * the section draw their own rails (Section.module.css).
 */

/** How far right of the main line the forked cards' rail sits. */
const COLUMN_GAP = 12;
/** The main line's x. */
export const MAIN_X = 18;
/** The forked cards' rail, and a moved-on target's. */
export const CARD_X = MAIN_X + COLUMN_GAP;
/** A rail's x inside a row: the row inset plus half the glyph. Keep in sync with Row.module.css. */
const GLYPH_X = 20;
/** The rows' own inset, Row.module.css's default. */
const ROW_INSET = 12;
/** The row inset that puts a row's glyph on the line at `x`. */
export const rowInsetFor = (x: number): number => x - (GLYPH_X - ROW_INSET);
/** Every turn is a quarter circle of this radius. */
const CORNER_R = 4;
/** The gap between cards, tall enough for a rail to bend through. Keep in sync with Section.module.css. */
export const CARD_GAP = 20;
/** The stuck merge base row's height, hairline and air included, which a row scrolled into view clears. Keep in sync with Section.module.css. */
export const DOCKED_HEIGHT = 1 + 4 + 28 + 4;
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

/** A target commit as a value, shared with the Upstream tab. The change id falls back to the commit id; revisit. */
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
	/** The target's tip is the base itself: one row stands for both. */
	refOnBase: boolean;
	incomingExpanded: boolean;
	baseExpanded: boolean;
	/** The commit the stacks nearest the tip sit on; the base header names it. Null while unknown. */
	base: TargetCommit | null;
	/** Commits on the target the workspace lacks, on their leg. Empty while folded. */
	incoming: Array<Run>;
	/** Below the base, on the main line; empty while the base is folded. */
	belowBase: Array<Item>;
	/** Older history below the deepest fork point, as much as is shown. Empty while the base is folded. */
	older: Array<TargetCommit>;
	/** Older history loaded but not shown yet: what the next ask reveals before any page is fetched. */
	olderHidden: number;
};

/** A stretch of the target line: a stack's base, or the run between such forks. */
type TargetItem =
	| { type: "fork"; commit: TargetCommit }
	| { type: "run"; commits: Array<TargetCommit>; inWorkspace: boolean };

/** Cut the target line at the stacks' bases; between them, maximal runs with one relation to the workspace. */
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
		// Incoming runs show their newest; runs the workspace has fold entirely.
		// A fold hiding one row is not worth it.
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
	// The listing's tail below the deepest fork heads the older history.
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

	// Deeper bases first; a base the listing does not reach counts as deepest,
	// keeping the order given.
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

/** An S-bend from one line to another: two quarter-turns joined by a straight. */
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
export const LEG_GAP = 12;
/** The leg's bend under the target's card: off the card's floor, onto the main line at the merge base header's top. */
export const LEG_BEND = `M ${n(CARD_X)} 0 ${sBend(CARD_X, 0, MAIN_X, LEG_GAP)}`;

/** A line straight down from `from` to `to`. */
const runDown = (rails: Array<string>, x: number, from: number, to: number): void => {
	if (to > from) rails.push(`M ${n(x)} ${n(from)} L ${n(x)} ${n(to)}`);
};

/**
 * The rails through the gaps between the cards: the main line from the top
 * card's floor down to the section, and each other card's bend onto it.
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
