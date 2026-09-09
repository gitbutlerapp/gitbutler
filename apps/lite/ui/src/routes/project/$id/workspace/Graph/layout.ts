import { addressEquals, commitAddress, type Address } from "#ui/addresses.ts";
import { assert } from "#ui/assert.ts";
import { remoteTrackingLabel } from "#ui/branch.ts";
import type { RefInfo, Stack, TargetCommit, TargetCommitPage, Worktree } from "@gitbutler/but-sdk";

/*
 * The stacks section as a graph: card order and which section rows show. Pure.
 *
 * One main line, the trunk, runs up the panel's edge from below the target's
 * row to the uncommitted files. Every stack card, and a moved-on target's, sits in
 * the column beside it and bends onto it in the gap under it. Rows draw their
 * own gutters, a column each for the lines behind them and the glyph
 * (GraphSegment); a card draws the gap under it.
 */

/**
 * The rows' inset in the graph. The trunk's column sits one 12px column left
 * of it, so its line, 8px in, is centred at x = 1 on the panel's edge, and the
 * first glyph column starts here. Whole pixels throughout: SVGs snap to them
 * where CSS boxes do not, and a fractional inset puts the two out of step. The
 * uncommitted files card carries no inset: the edge column is its whole
 * gutter (GraphEdge).
 */
export const ROW_INSET = 12 - 8 + 1;
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
/** A long run shows this much at first, and this much more with each ask. */
const FIRST = 10;
const MORE = 20;

type Folds = {
	/** The target header's fold: the commits incoming from the target. */
	incomingExpanded: boolean;
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
		const asFar = FIRST + asked * MORE;
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

export const layout = (
	stacks: ReadonlyArray<Stack>,
	target: RefInfo["target"],
	listing: TargetCommitPage | undefined,
	folds: Folds,
	worktrees: ReadonlyArray<Worktree> = [],
): Plan => {
	const commits = listing?.commits ?? [];
	const line = segmentAtForks(commits, stacks);
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
		order: order.map((entry) => entry.index),
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
		incoming: folds.incomingExpanded ? incomingRuns(line, folds) : [],
		worktrees: placeWorktrees(stacks, worktrees),
	};
};

/** The section's rows as values, top to bottom, as the fold state shows them. The header is not a value. */
export const sectionAddresses = (plan: Plan): Array<Address> =>
	plan.incoming.flatMap((run) => run.shown.map(targetCommitAddress));

/** The review a commit on the target line landed, by commit id; null when none or not on the line. */
export const targetCommitReview = (listing: TargetCommitPage | undefined, commitId: string) =>
	listing?.commits.find((commit) => commit.commit.id === commitId)?.review ?? null;

/** Whether a row sits in the section's fold, so a fold key on the row can close it. */
export const inSection = (plan: Plan, address: Address): boolean =>
	sectionAddresses(plan).some((other) => addressEquals(address, other));
