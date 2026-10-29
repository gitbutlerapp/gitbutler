import { branchRequiresForcePush } from "$lib/stacks/stack";
import type { Segment } from "@gitbutler/but-sdk";

/**
 * Values derived from a segment's position in a stack. Pre-computed by
 * parent components (BranchList iteration, StackDetails selection) so
 * that leaves can take narrow props instead of the whole segments array.
 */
export interface SegmentContext {
	branchIndex: number;
	parent: Segment | undefined;
	child: Segment | undefined;
	withForce: boolean;
}

/**
 * Stack-wide data shared by every SegmentContext in a stack. Compute
 * this ONCE per stack (e.g. via `$derived`) and pass it to
 * `segmentContext(...)` for each index — that keeps the per-segment work
 * O(1) instead of O(N) per call.
 */
export interface StackPrecomputed {
	/**
	 * `withForceFromIndex[i]` is true when *any* segment at index `>= i`
	 * needs a force push. Pre-computed via a single suffix-OR pass over
	 * the stack so per-segment lookup is O(1).
	 */
	withForceFromIndex: boolean[];
}

export function precomputeStack(segments: Segment[]): StackPrecomputed {
	const withForceFromIndex = new Array<boolean>(segments.length);
	let anyForceFromHere = false;
	for (let i = segments.length - 1; i >= 0; i--) {
		const s = segments[i];
		if (s && branchRequiresForcePush(s)) anyForceFromHere = true;
		withForceFromIndex[i] = anyForceFromHere;
	}

	return { withForceFromIndex };
}

export function segmentContext(
	segments: Segment[],
	index: number,
	precomputed: StackPrecomputed,
): SegmentContext {
	return {
		branchIndex: index,
		parent: segments[index + 1],
		child: segments[index - 1],
		withForce: precomputed.withForceFromIndex[index] ?? false,
	};
}
