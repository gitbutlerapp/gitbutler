import { branchListQueryOptions } from "#ui/api/queries.ts";
import { unappliedStacks } from "#ui/branch.ts";
import { EmptyState } from "@gitbutler/ui-react/EmptyState.tsx";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { useQuery } from "@tanstack/react-query";
import type { FC } from "react";

/**
 * The stacks panel with nothing applied, which is two states rather than one.
 *
 * A project with no branches at all needs no rescue: committing creates one on
 * its own, and the line says so. A project whose branches are simply elsewhere
 * says how many are waiting. Both are an empty section where the cards would
 * be, text only: the panel's header and the Branches tab already carry the
 * ways to start or pick one, so the section doesn't repeat them.
 */
export const NoStacks: FC<{ projectId: string }> = ({ projectId }) => {
	// The same filters the branches page lists under, so the count promises
	// exactly what "See all" then shows.
	const filters = useAppSelector((state) =>
		projectSlice.selectors.selectBranchFilters(state, projectId),
	);
	const { data: unappliedBranchCount } = useQuery({
		...branchListQueryOptions(projectId),
		// Derived in `select` so react-query caches it against the branch list
		// rather than recounting every render.
		select: (listedStacks) =>
			unappliedStacks(listedStacks, filters).reduce(
				(count, stack) => count + stack.branches.length,
				0,
			),
	});

	// Not loaded is not the same as nothing to report: rendering early would
	// flash the fresh-project wording at every project that has branches.
	if (unappliedBranchCount === undefined) return null;

	const hasBranchesElsewhere = unappliedBranchCount > 0;

	return (
		<EmptyState
			align="start"
			title={hasBranchesElsewhere ? "No branches applied" : "No branches yet"}
			description={
				hasBranchesElsewhere
					? `You have ${unappliedBranchCount} ${unappliedBranchCount === 1 ? "branch" : "branches"} to pick from`
					: "Your first commit will start one"
			}
		/>
	);
};
