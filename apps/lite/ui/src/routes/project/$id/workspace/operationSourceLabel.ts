import { findCommit, findSegmentByBranchRef } from "#ui/api/ref-info.ts";
import { commitTitle, shortCommitId } from "#ui/commit.ts";
import { Match } from "effect";
import { type RefInfo } from "@gitbutler/but-sdk";
import { Operand } from "#ui/operands.ts";
import { formatHunkHeader } from "#ui/hunk.ts";
import { assert } from "#ui/assert.ts";

export const operationSourceLabel = ({
	source,
	headInfo,
}: {
	source: Operand;
	headInfo: RefInfo;
}) =>
	Match.value(source).pipe(
		Match.tagsExhaustive({
			Branch: ({ branchRef }) => {
				const segment = findSegmentByBranchRef({ headInfo, branchRef });
				return assert(segment?.refName).displayName;
			},
			File: ({ path }) => path,
			ChangesSection: () => "Changes",
			Commit: ({ commitId }) => {
				const commit = findCommit({ headInfo, commitId });
				return commit
					? `${commitTitle(commit.message) ?? "(no message)"}${commit.hasConflicts ? " ⚠️" : ""}`
					: shortCommitId(commitId);
			},
			Stack: () => "Stack",
			Hunk: ({ hunkHeader }) => `Hunk ${formatHunkHeader(hunkHeader)}`,
		}),
	);
