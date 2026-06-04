import { partialStackRequestsForcePush } from "$lib/stacks/stack";
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
	stackPrNumbers: (number | undefined)[];
}

export function segmentContext(segments: Segment[], index: number): SegmentContext {
	const branchName = segments[index]?.refName?.displayName;
	return {
		branchIndex: index,
		parent: segments[index + 1],
		child: segments[index - 1],
		withForce: branchName ? partialStackRequestsForcePush(branchName, segments) : false,
		stackPrNumbers: segments.map((s) => s.metadata?.review.pullRequest ?? undefined),
	};
}
