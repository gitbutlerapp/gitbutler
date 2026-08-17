import type { HeadInfoIndex } from "#ui/api/ref-info.ts";
import { commitTitle, shortCommitId } from "#ui/commit.ts";
import { Match } from "effect";
import type { Operand } from "#ui/operands.ts";
import { assert } from "#ui/assert.ts";

const hunkLabel = (operands: Array<Extract<Operand, { _tag: "Hunk" }>>) => {
	const add = operands.reduce(
		(sum, operand) =>
			sum +
			operand.lineGroups.reduce(
				(groupSum, group) => groupSum + (group.side === "additions" ? group.lines : 0),
				0,
			),
		0,
	);
	const del = operands.reduce(
		(sum, operand) =>
			sum +
			operand.lineGroups.reduce(
				(groupSum, group) => groupSum + (group.side === "deletions" ? group.lines : 0),
				0,
			),
		0,
	);
	const all = add + del;

	// Probably shouldn't happen?
	if (all == 0) return "0 changed lines";

	let words = "";
	if (add > 0) words += `+${add}`;
	if (add > 0 && del > 0) words += ` `;
	if (del > 0) words += `-${del}`;
	if (all === 1) words += " line";
	else words += " lines";
	return words;
};

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
			Commit: ({ commitId }) => {
				const commit = headInfoIndex.commitContextByCommitId(commitId)?.commit;
				return commit
					? `${commitTitle(commit.message) ?? "(no message)"}${commit.hasConflicts ? " ⚠️" : ""}`
					: shortCommitId(commitId);
			},
			Hunk: (operand) => hunkLabel([operand]),
		}),
	);

export const operandsLabel = ({
	operands,
	headInfoIndex,
}: {
	operands: Array<Operand>;
	headInfoIndex: HeadInfoIndex;
}) => {
	if (operands.length > 0 && operands.every((operand) => operand._tag === "Hunk"))
		return hunkLabel(operands);
	if (operands.length !== 1) return `${operands.length.toLocaleString()} items`;

	return operandLabel({ operand: assert(operands[0]), headInfoIndex });
};
