import type { BottomUpdate, RelativeTo, Segment, Stack } from "@gitbutler/but-sdk";

export const segmentBottomRelativeTo = (segment: Segment): RelativeTo | null => {
	const bottomCommit = segment.commits.at(-1);
	if (bottomCommit) return { type: "commit", subject: bottomCommit.id };

	const bottomRef = segment.refName?.fullNameBytes;
	if (bottomRef) return { type: "referenceBytes", subject: bottomRef };

	return null;
};

export const stackToBottomRebaseUpdate = (stack: Stack): BottomUpdate | null => {
	const bottomSegment = stack.segments.at(-1);
	if (!bottomSegment) return null;

	const relativeTo = segmentBottomRelativeTo(bottomSegment);
	if (relativeTo) return { kind: "rebase", selector: relativeTo };

	return null;
};
