import type { BottomUpdate, Stack } from "@gitbutler/but-sdk";

export const stackToBottomRebaseUpdate = (stack: Stack): BottomUpdate | null => {
	const bottomSegment = stack.segments.at(-1);
	if (!bottomSegment) return null;

	const bottomCommit = bottomSegment.commits.at(-1);
	if (bottomCommit)
		return {
			kind: "rebase",
			selector: { type: "commit", subject: bottomCommit.id },
		};

	const bottomRef = bottomSegment.refName?.fullNameBytes;
	if (bottomRef)
		return {
			kind: "rebase",
			selector: { type: "referenceBytes", subject: bottomRef },
		};

	return null;
};
