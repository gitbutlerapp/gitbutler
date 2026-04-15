import { findCommit, findSegmentByBranchRef } from "#ui/domain/RefInfo.ts";
import {
	CommitLabel,
	decodeRefName,
	formatHunkHeader,
	shortCommitId,
} from "#ui/routes/project/$id/shared.tsx";
import { Match } from "effect";
import { type FC } from "react";
import { type RefInfo } from "@gitbutler/but-sdk";
import { type OperationSource } from "./OperationSource.ts";

export const OperationSourceLabel: FC<{
	source: OperationSource;
	headInfo: RefInfo;
}> = ({ source, headInfo }) =>
	Match.value(source).pipe(
		Match.tagsExhaustive({
			Stack: () => "Stack",
			Segment: ({ branchRef }) => {
				const segment = findSegmentByBranchRef({ headInfo, branchRef });
				if (segment?.refName) return segment.refName.displayName;
				if (branchRef) return decodeRefName(branchRef);
				return "Segment";
			},
			BaseCommit: () => "Base commit",
			Commit: ({ commitId }) => {
				const commit = findCommit({ headInfo, commitId });
				return commit ? <CommitLabel commit={commit} /> : shortCommitId(commitId);
			},
			ChangesSection: () => "Changes",
			File: ({ path }) => path,
			Hunk: ({ hunkHeader }) => `Hunk ${formatHunkHeader(hunkHeader)}`,
		}),
	);
