import { findCommit, findSegmentByBranchRef } from "#ui/domain/RefInfo.ts";
import {
	assert,
	CommitLabel,
	formatHunkHeader,
	shortCommitId,
} from "#ui/routes/project/$id/shared.tsx";
import { Match } from "effect";
import { type FC } from "react";
import { type RefInfo } from "@gitbutler/but-sdk";
import { Item } from "./Item";

export const OperationSourceLabel: FC<{
	source: Item;
	headInfo: RefInfo;
}> = ({ source, headInfo }) =>
	Match.value(source).pipe(
		Match.tagsExhaustive({
			BaseCommit: () => "Base commit",
			Branch: ({ branchRef }) => {
				const segment = findSegmentByBranchRef({ headInfo, branchRef });
				return assert(segment?.refName).displayName;
			},
			ChangeFile: ({ treeChange }) => treeChange.path,
			ChangesSection: () => "Changes",
			Commit: ({ commitId }) => {
				const commit = findCommit({ headInfo, commitId });
				return commit ? <CommitLabel commit={commit} /> : shortCommitId(commitId);
			},
			CommitFile: ({ treeChange }) => treeChange.path,
			Stack: () => "Stack",
			Hunk: ({ treeChange }) =>
				treeChange.hunkHeaders
					.map((hunkHeader) => `Hunk ${formatHunkHeader(hunkHeader)}`)
					.join(", "),
		}),
	);
