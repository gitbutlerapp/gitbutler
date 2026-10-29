import type { InsertSide, RelativeTo } from "@gitbutler/but-sdk";

export function normalizeReferenceSubject(referenceName: string): string {
	return referenceName.startsWith("refs/") ? referenceName : `refs/heads/${referenceName}`;
}

export function toCommitMovePlacement(args: {
	targetBranchName: string;
	targetCommitId: string | "top";
}): {
	relativeTo: RelativeTo;
	side: InsertSide;
} {
	if (args.targetCommitId === "top") {
		return {
			// `reference + below` inserts the commit as the new branch tip.
			relativeTo: {
				type: "reference",
				subject: normalizeReferenceSubject(args.targetBranchName),
			},
			side: "below",
		};
	}

	return {
		relativeTo: {
			type: "commit",
			subject: args.targetCommitId,
		},
		side: "below",
	};
}
