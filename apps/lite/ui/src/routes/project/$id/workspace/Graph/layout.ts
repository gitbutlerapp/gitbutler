import { addressEquals, commitAddress, type Address } from "#ui/addresses.ts";
import { assert } from "#ui/assert.ts";
import { remoteTrackingLabel } from "#ui/branch.ts";
import { GRAPH_LANE_WIDTH, GRAPH_TRUNK_INSET } from "#ui/components/graph-spacing.ts";
import type { RefInfo, Stack, TargetCommit, TargetCommitPage, Worktree } from "@gitbutler/but-sdk";

/*
 * The stacks section as a graph: card order and which section rows show. Pure.
 *
 * One main line, the trunk, runs up near the panel's edge from below the target's
 * row to the uncommitted files. Every stack card, and a moved-on target's, sits in
 * the column beside it and bends onto it in the gap under it. Rows draw their
 * own gutters, a column each for the lines behind them and the glyph
 * (GraphSegment); a card draws the gap under it.
 */

/**
 * GraphEdge's line is 1px into its canvas; regular glyphs are 8px in.
 * Leave one lane between them while keeping the trunk at its shared inset.
 */
export const ROW_INSET = GRAPH_TRUNK_INSET + 1 + GRAPH_LANE_WIDTH - 8;
/** The gap under a card, tall enough for a line to bend through. */
export const CARD_GAP = 20;
/** The gap under a worktree lane, which its line bends through. */
export const LEG_GAP = 12;
/** The connector between a worktree on a branch's tip and the branch row under it. */
export const TIP_GAP = 8;
/** The stuck target row's height, hairline and air included, which a row scrolled into view clears. Keep in sync with Section.module.css. */
export const DOCKED_HEIGHT = 1 + 4 + 28 + 4;
/** The stuck uncommitted files row's height: the card's head room, a row and a hairline. Keep in sync with WorkspaceLists.module.css. */
export const HEAD_DOCKED_HEIGHT = 6 + 28 + 1;
const FIRST_INCOMING = 10;
const FIRST_HISTORY = 5;
/** Additional commits revealed with each ask. */
export const MORE_COMMITS = 20;

type Folds = {
	/** The target header's fold: the commits incoming from the target. */
	incomingExpanded: boolean;
	historyExpanded: boolean;
	moreHistory: number;
	/** How many times more of each run was asked for, by the run's id. */
	moreRuns: Readonly<Record<string, number>>;
};

/** A target commit as a value. The change id falls back to the commit id; revisit. */
export const targetCommitAddress = (commit: TargetCommit): Address =>
	commitAddress({
		commitId: commit.commit.id,
		changeId: commit.commit.changeId ?? commit.commit.id,
	});

/** A run of commits on the target the workspace lacks. */
export type Run = {
	id: string;
	/** The commits shown, newest first. */
	shown: Array<TargetCommit>;
	/** How many more it holds, folded away below them. */
	hidden: number;
	/** Whether an ask revealed more than shows at first. */
	expanded: boolean;
};

/**
 * Where the linked worktrees go, the way `but status` places them: a lane
 * nested above the commit it rests on when a card or another worktree shows
 * that commit, otherwise a lane of its own under the cards.
 */
export type WorktreePlacement = {
	/** The worktrees resting on each shown commit, by its id, in the order given. */
	on: ReadonlyMap<string, ReadonlyArray<Worktree>>;
	/** Resting below the workspace, on a commit nothing shows, or on unknown history. */
	standalone: ReadonlyArray<Worktree>;
};

export type Plan = {
	/** Card order as indices into the stacks given: by base depth, deepest first, then as given. */
	order: Array<number>;
	/**
	 * The target's row, standing for the workspace's base: its label, how many
	 * commits are incoming, and whether the base is at the target's tip. Null
	 * without a target, and until its commits are known. Not a value: nothing
	 * selects it.
	 */
	header: { label: string; incoming: number; current: boolean } | null;
	incomingExpanded: boolean;
	/** Commits on the target the workspace lacks, on their leg. Empty while folded. */
	incoming: Array<Run>;
	historyAvailable: boolean;
	historyExpanded: boolean;
	/** The workspace's target history, starting at its newest shared commit. */
	history: Array<TargetCommit>;
	historyHidden: number;
	worktrees: WorktreePlacement;
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

/** The runs the workspace lacks, each showing its newest and more with each ask. A fold hiding one row is not worth it. */
const incomingRuns = (line: ReadonlyArray<TargetItem>, folds: Folds): Array<Run> =>
	line.flatMap((item) => {
		if (item.type === "fork" || item.inWorkspace) return [];
		const id = assert(item.commits[0]).commit.id;
		const asked = folds.moreRuns[id] ?? 0;
		const asFar = FIRST_INCOMING + asked * MORE_COMMITS;
		const count = item.commits.length - asFar <= 1 ? item.commits.length : asFar;
		return [
			{
				id,
				shown: item.commits.slice(0, count),
				hidden: item.commits.length - count,
				expanded: asked > 0,
			},
		];
	});

const placeWorktrees = (
	stacks: ReadonlyArray<Stack>,
	worktrees: ReadonlyArray<Worktree>,
): WorktreePlacement => {
	// A worktree's own commits count as shown, so a worktree resting on another
	// nests inside its lane. Bases are assigned in tip order, so this cannot cycle.
	const shown = new Set<string>([
		...stacks.flatMap((stack) =>
			stack.segments.flatMap((segment) => segment.commits.map((commit) => commit.id)),
		),
		...worktrees.flatMap((worktree) =>
			worktree.segments.flatMap((segment) => segment.commits.map((commit) => commit.id)),
		),
	]);
	const on = new Map<string, Array<Worktree>>();
	const standalone: Array<Worktree> = [];
	for (const worktree of worktrees) {
		const base = worktree.base?.subject;
		if (base === undefined || !shown.has(base)) standalone.push(worktree);
		else on.set(base, [...(on.get(base) ?? []), worktree]);
	}
	return { on, standalone };
};

/**
 * The worktrees resting on the tip of `stack`'s top branch. They are drawn
 * above that branch row, continuing the stack's line the way a branch stacked
 * on top would, rather than forking off the commit: their commits descend from
 * it and nothing else in the card does. Lower down, a fork is the truth.
 */
export const worktreesOnTip = (
	worktrees: WorktreePlacement,
	stack: Stack,
): ReadonlyArray<Worktree> => {
	const top = stack.segments[0];
	const tip = top?.commits[0];
	if (top === undefined || top.refName === null || tip === undefined) return [];
	return worktrees.on.get(tip.id) ?? [];
};

/** A clipped listing may not have reached the shared base yet. */
export const canLoadHistory = (listing: TargetCommitPage | undefined): boolean =>
	listing !== undefined &&
	(listing.hasMore || listing.commits.some((commit) => commit.inWorkspace));

/** Stack placement is independent of the folds and older History pages. */
export const layoutStructure = (
	stacks: ReadonlyArray<Stack>,
	listing: TargetCommitPage | undefined,
	worktrees: ReadonlyArray<Worktree>,
) => {
	const line = segmentAtForks(listing?.commits ?? [], stacks);
	const forkOrder = line.flatMap((item) => (item.type === "fork" ? [item.commit.commit.id] : []));
	// Deeper bases first; a base the listing does not reach counts as deepest,
	// keeping the order given.
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
		line,
		order: order.map((entry) => entry.index),
		worktrees: placeWorktrees(stacks, worktrees),
	};
};

export const layout = (
	stacks: ReadonlyArray<Stack>,
	target: RefInfo["target"],
	listing: TargetCommitPage | undefined,
	folds: Folds,
	worktrees: ReadonlyArray<Worktree> = [],
	olderPages: ReadonlyArray<TargetCommit> = [],
	structure = layoutStructure(stacks, listing, worktrees),
): Plan => {
	const commits = listing?.commits ?? [];
	const shared = commits.filter((commit) => commit.inWorkspace);
	const historyAvailable = target !== null && canLoadHistory(listing);
	const history =
		historyAvailable && folds.historyExpanded
			? [...shared, ...olderPages.filter((commit) => commit.inWorkspace)]
			: [];
	const historyCount = FIRST_HISTORY + folds.moreHistory * MORE_COMMITS;

	return {
		order: structure.order,
		header:
			target === null || listing === undefined
				? null
				: {
						label: remoteTrackingLabel(target.remoteTrackingRef),
						incoming: commits.filter((entry) => !entry.inWorkspace).length,
						// The stored target trails the fetched tip exactly while there is work
						// to do; counting incoming commits misses the case where a lane already
						// holds them.
						current: target.isCurrent,
					},
		incomingExpanded: folds.incomingExpanded,
		incoming: folds.incomingExpanded ? incomingRuns(structure.line, folds) : [],
		historyAvailable,
		historyExpanded: folds.historyExpanded,
		history: history.slice(0, historyCount),
		historyHidden: Math.max(0, history.length - historyCount),
		worktrees: structure.worktrees,
	};
};

/** The section's rows as values, top to bottom, as the fold state shows them. The header is not a value. */
export const foldAddresses = (plan: Plan, fold: "incoming" | "history"): Array<Address> =>
	(fold === "incoming" ? plan.incoming.flatMap((run) => run.shown) : plan.history).map(
		targetCommitAddress,
	);

export const sectionAddresses = (plan: Plan): Array<Address> => [
	...foldAddresses(plan, "incoming"),
	...foldAddresses(plan, "history"),
];

/** The review a commit on the target line landed, by commit id; null when none or not on the line. */
export const targetCommitReview = (
	listing: TargetCommitPage | undefined,
	commitId: string,
	history: ReadonlyArray<TargetCommit> = [],
) =>
	listing?.commits.find((commit) => commit.commit.id === commitId)?.review ??
	history.find((commit) => commit.commit.id === commitId)?.review ??
	null;

/** Whether a row sits in the section's fold, so a fold key on the row can close it. */
export const foldAt = (plan: Plan, address: Address): "incoming" | "history" | null => {
	const inFold = (fold: "incoming" | "history") =>
		foldAddresses(plan, fold).some((other) => addressEquals(address, other));
	return inFold("incoming") ? "incoming" : inFold("history") ? "history" : null;
};
