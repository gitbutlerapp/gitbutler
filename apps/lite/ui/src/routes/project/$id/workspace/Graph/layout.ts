import { addressEquals, commitAddress, type Address } from "#ui/addresses.ts";
import { assert } from "#ui/assert.ts";
import { remoteTrackingLabel } from "#ui/branch.ts";
import type { RefInfo, Stack, TargetCommit, TargetCommitPage } from "@gitbutler/but-sdk";

/*
 * The stacks section as a graph: card order and which section rows show. Pure.
 *
 * One main line, the trunk, runs up the left from the merge base to the
 * uncommitted files. Every stack card, and a moved-on target's, sits a column
 * to its right and bends onto it in the gap under it. Rows draw their own
 * gutters, a column each for the lines behind them and the glyph
 * (GraphSegment); a card draws the gap under it.
 */

/** The rows' inset in the graph: the first column's line, 8px in, at x = 18. */
export const ROW_INSET = 10;
/** The gap under a card, tall enough for a line to bend through. */
export const CARD_GAP = 20;
/** The gap under the target's card and the ref row, which the leg bends through. */
export const LEG_GAP = 12;
/** The stuck merge base row's height, hairline and air included, which a row scrolled into view clears. Keep in sync with Section.module.css. */
export const DOCKED_HEIGHT = 1 + 4 + 28 + 4;
/** The stuck uncommitted files row's height: the card's head room, a row and a hairline. Keep in sync with WorkspaceLists.module.css. */
export const HEAD_DOCKED_HEIGHT = 6 + 28 + 1;
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

/** A target commit as a value. The change id falls back to the commit id; revisit. */
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
	/** No target: the sole stack is the main line, so it runs on the trunk instead of a column off it. */
	stackOnTrunk: boolean;
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
		stackOnTrunk: target === null && stacks.length === 1,
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
