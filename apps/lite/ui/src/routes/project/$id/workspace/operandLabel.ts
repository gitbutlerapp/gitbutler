import { findCommit, findSegmentByBranchRef } from "#ui/api/ref-info.ts";
import { commitTitle, shortCommitId } from "#ui/commit.ts";
import { Match } from "effect";
import { type RefInfo } from "@gitbutler/but-sdk";
import { Operand } from "#ui/operands.ts";
import { assert } from "#ui/assert.ts";

export const operandLabel = ({ operand, headInfo }: { operand: Operand; headInfo: RefInfo }) =>
	Match.value(operand).pipe(
		Match.tagsExhaustive({
			Branch: ({ branchRef }) => {
				const segment = findSegmentByBranchRef({ headInfo, branchRef });
				return assert(segment?.refName).displayName;
			},
			File: ({ path }) => path,
			UncommittedChanges: () => "Uncommitted changes",
			Commit: ({ commitId }) => {
				const commit = findCommit({ headInfo, commitId });
				return commit
					? `${commitTitle(commit.message) ?? "(no message)"}${commit.hasConflicts ? " ⚠️" : ""}`
					: shortCommitId(commitId);
			},
			Stack: () => "Stack",
			Hunk: ({ lineGroups }) => {
				const count = lineGroups.reduce((sum, group) => sum + group.lines, 0);
				return `${count} changed line${count !== 1 ? "s" : ""}`;
			},
		}),
	);
