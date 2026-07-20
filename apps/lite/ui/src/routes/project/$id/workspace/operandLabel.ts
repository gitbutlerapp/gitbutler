import type { HeadInfoIndex } from "#ui/api/ref-info.ts";
import { commitTitle, shortCommitId } from "#ui/commit.ts";
import { Match } from "effect";
import { Operand } from "#ui/operands.ts";
import { assert } from "#ui/assert.ts";

export const operandLabel = ({
	operand,
	headInfoIndex,
}: {
	operand: Operand;
	headInfoIndex: HeadInfoIndex;
}) =>
	Match.value(operand).pipe(
		Match.tagsExhaustive({
			Branch: ({ branchRef }) => {
				const segment = headInfoIndex.branchContextByRefBytes(branchRef)?.segment;
				return assert(segment?.refName).displayName;
			},
			File: ({ path }) => path,
			UncommittedChanges: () => "Uncommitted changes",
			Commit: ({ changeId }) => {
				const commit = headInfoIndex.commitContextById(changeId)?.commit;
				return commit
					? `${commitTitle(commit.message) ?? "(no message)"}${commit.hasConflicts ? " ⚠️" : ""}`
					: shortCommitId(changeId);
			},
			Stack: () => "Stack",
			Hunk: ({ lineGroups }) => {
				const count = lineGroups.reduce((sum, group) => sum + group.lines, 0);
				return `${count} changed line${count !== 1 ? "s" : ""}`;
			},
		}),
	);

export const operandsLabel = ({
	operands,
	headInfoIndex,
}: {
	operands: Array<Operand>;
	headInfoIndex: HeadInfoIndex;
}) => {
	if (operands.length !== 1) return `${operands.length.toLocaleString()} items`;

	return operandLabel({ operand: assert(operands[0]), headInfoIndex });
};
