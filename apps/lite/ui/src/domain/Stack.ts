import { type RelativeTo, type Stack } from "@gitbutler/but-sdk";

export const stackRelativeTo = (stack: Stack): RelativeTo | null => {
	const segmentWithRef = stack.segments.find((segment) => segment.refName != null);
	if (segmentWithRef?.refName)
		return {
			type: "referenceBytes",
			subject: segmentWithRef.refName.fullNameBytes,
		};

	const firstCommit = stack.segments.flatMap((segment) => segment.commits)[0];
	if (!firstCommit) return null;

	return { type: "commit", subject: firstCommit.id };
};
