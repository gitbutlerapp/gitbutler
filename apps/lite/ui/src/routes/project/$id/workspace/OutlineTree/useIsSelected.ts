import { NavigationIndexContext } from "../OutlineNavigationIndexContext.ts";
import {
	branchOperand,
	commitOperand,
	operandEquals,
	operandIdentityKey,
	type Operand,
} from "#ui/operands.ts";
import { selectProjectSelectionOutline } from "#ui/projects/state.ts";
import { resolveNavigationIndexSelection } from "#ui/selection-scopes.ts";
import { useAppSelector } from "#ui/store.ts";
import { assert } from "#ui/assert.ts";
import { use } from "react";
import { Match } from "effect";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { resolveCommit } from "#ui/commit.ts";
import { useQuery } from "@tanstack/react-query";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { resolveBranch } from "#ui/segment.ts";

export const useIsSelected = ({
	projectId,
	operand,
}: {
	projectId: string;
	operand: Operand;
}): boolean => {
	const { data: headInfoIndex } = useQuery({
		...headInfoQueryOptions(projectId),
		select: getHeadInfoIndex,
	});

	const navigationIndex = assert(use(NavigationIndexContext));

	return useAppSelector((state) => {
		const selection = selectProjectSelectionOutline(state, projectId);

		const resolved = Match.value(selection).pipe(
			Match.tags({
				Commit: (commit) => {
					const res = headInfoIndex && resolveCommit(headInfoIndex, commit);
					return res ? commitOperand(res) : null;
				},
				Branch: (branch) => {
					const res = headInfoIndex && resolveBranch(headInfoIndex, branch);
					return res ? branchOperand(res) : null;
				},
			}),
			Match.orElse(() => selection),
		);

		const filtered = resolveNavigationIndexSelection(navigationIndex, resolved, operandIdentityKey);

		return filtered ? operandEquals(filtered, operand) : false;
	});
};
