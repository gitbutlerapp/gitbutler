import type {
	BottomUpdate,
	RefInfo,
	RelativeTo,
	Segment,
	Stack,
	Worktree,
} from "@gitbutler/but-sdk";

export const segmentBottomRelativeTo = (segment: Segment): RelativeTo | null => {
	const bottomCommit = segment.commits.at(-1);
	if (bottomCommit) return { type: "commit", subject: bottomCommit.id };

	const bottomRef = segment.refName?.fullNameBytes;
	if (bottomRef) return { type: "referenceBytes", subject: bottomRef };

	return null;
};

export const stackBottomRelativeTo = (stack: Stack): RelativeTo | null => {
	const bottomSegment = stack.segments.at(-1);
	if (!bottomSegment) return null;

	const relativeTo = segmentBottomRelativeTo(bottomSegment);
	if (relativeTo) return relativeTo;

	return null;
};

/** A worktree stacked on a branch has no bottom of its own: it moves with that branch. */
const worktreeBottomRelativeTo = (worktree: Worktree): RelativeTo | null => {
	if (worktree.base?.type !== "Outside") return null;

	const bottomSegment = worktree.segments.at(-1);
	return bottomSegment ? segmentBottomRelativeTo(bottomSegment) : null;
};

/** One rebase for every stack and worktree resting on the target. */
export const rebaseAllUpdates = (headInfo: RefInfo): Array<BottomUpdate> =>
	[
		...headInfo.stacks.map(stackBottomRelativeTo),
		...headInfo.worktrees.map(worktreeBottomRelativeTo),
	]
		.filter((relativeTo) => relativeTo != null)
		.map((relativeTo) => ({ kind: "rebase", selector: relativeTo }));
