import type { Stack, Worktree } from "@gitbutler/but-sdk";

/** The lane id of the stack without an id, shown outside workspace mode. */
export const BRANCHLESS_LANE_ID = "banana";

/**
 * The lane id of a linked git worktree shown in the workspace: its stable
 * name, under a prefix that keeps it apart from stack ids (UUIDs) and the
 * "banana" lane, so per-lane UI state never collides.
 *
 * It is only ever a lane id, never a stack id: stack ids are sent to the
 * backend, and a worktree's branches have none.
 */
export function worktreeLaneId(name: string): string {
	return `worktree:${name}`;
}

/**
 * The lane something was selected in: its linked worktree's, or its stack's.
 * Undefined for the branchless lane, which can't rewrite its commits.
 */
export function laneIdOf(args: { stackId?: string; worktree?: string }): string | undefined {
	if (args.worktree !== undefined) return worktreeLaneId(args.worktree);
	return args.stackId;
}

/** A linked worktree's own branches as a stack without an id, resting on its base. */
export function worktreeStack(worktree: Worktree): Stack {
	return { id: null, base: worktree.base?.subject ?? null, segments: worktree.segments };
}
