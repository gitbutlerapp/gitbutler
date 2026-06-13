import { findCommit, findSegmentByBranchRef } from "#ui/api/ref-info.ts";
import { commitTitle, shortCommitId } from "#ui/commit.ts";
import { Match } from "effect";
import { type RefInfo } from "@gitbutler/but-sdk";
import { Operand } from "#ui/operands.ts";
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
					? `${commitTitle(commit.message)}${commit.hasConflicts ? " ⚠️" : ""}`
					: shortCommitId(commitId);
			},
			Stack: () => "Stack",
			Hunk: ({ segments }) => {
				const count = segments.reduce(
					(sum, segment) =>
						sum + segment.lineGroups.reduce((segmentSum, group) => segmentSum + group.lines, 0),
					0,
				);
				return `${count} changed line${count !== 1 ? "s" : ""}`;
			},
		}),
	);
