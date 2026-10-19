import { BRANCHLESS_LANE_ID, worktreeLaneId, worktreeStack } from "$lib/linkedWorktrees/lane";
import type { Stack, Worktree } from "@gitbutler/but-sdk";

/** A lane of the workspace: a stack, or a linked worktree's branches as a stack without an id. */
export type Lane = {
	laneId: string;
	stack: Stack;
	worktree?: Worktree;
	/** The stack branch holding the commit `worktree` rests on, if it is named. */
	baseBranchName?: string;
	/** Position among all lanes, left to right. */
	index: number;
};

type UnnumberedLane = Omit<Lane, "index">;

export type WorkspaceLanes = {
	/** Each stack's lane, then the lanes of the worktrees resting on its commits. */
	groups: { stack: Stack; lanes: Lane[] }[];
	/** Worktrees resting outside the workspace, or on nothing a stack shows. */
	standalone: Lane[];
	count: number;
};

/**
 * Lay out the workspace's lanes. Each worktree goes next to the stack that
 * holds the commit it rests on, the way Lite draws a worktree off that
 * commit, and moves with it; the rest stand on their own after the stacks.
 */
export function workspaceLanes(stacks: Stack[], worktrees: Worktree[]): WorkspaceLanes {
	const groups = stacks.map((stack) => {
		const lanes: UnnumberedLane[] = [{ laneId: stack.id || BRANCHLESS_LANE_ID, stack }];
		return { stack, lanes };
	});
	// The stack segment holding the commit `worktree` rests on, if a stack shows it.
	function holderOf(worktree: Worktree) {
		if (worktree.base?.type !== "InWorkspace") return undefined;
		const base = worktree.base.subject;
		for (const group of groups) {
			if (group.stack.id === null) continue;
			const segment = group.stack.segments.find((segment) =>
				segment.commits.some((commit) => commit.id === base),
			);
			if (segment) return { group, segment };
		}
		return undefined;
	}

	const standalone: UnnumberedLane[] = [];
	for (const worktree of worktrees) {
		const holder = holderOf(worktree);
		const lane: UnnumberedLane = {
			laneId: worktreeLaneId(worktree.name),
			stack: worktreeStack(worktree),
			worktree,
			baseBranchName: holder?.segment.refName?.displayName,
		};
		(holder?.group.lanes ?? standalone).push(lane);
	}

	let count = 0;
	function numbered(lane: UnnumberedLane): Lane {
		return { ...lane, index: count++ };
	}
	const numberedGroups = groups.map(({ stack, lanes }) => ({ stack, lanes: lanes.map(numbered) }));
	const numberedStandalone = standalone.map(numbered);
	return { groups: numberedGroups, standalone: numberedStandalone, count };
}
